# 15. Execute Mammoth

Mammoth is the larger decision model. It has the same hybrid layer pattern,
tokenizer, prompt format and readout as Elephant, so this chapter does not
add a new engine. It gives the same engine Mammoth's dimensions, its two
checkpoint shards and its own memory plan, then runs the chapter 14 decision
through all 64 layers.

Start with the [target example](../examples/mammoth-target.coax). It runs
without downloading model weights:

```shell
coax/bin/coax run psionic/examples/mammoth-target.coax
```

```jai
model: baselabs/mammoth-1
layers: 48 linear, 16 full attention, width 5120
vocabulary: 248320, tensors: 1184, shards: 2
shard files: 54713608144 bytes
86-token request: 42041344 state floats, 218486272 workspace floats
```

<!-- source: psionic/examples/mammoth-target.coax example -->
```jai
main :: () {
    target:=ps_target(1,86);
    #assert target.status==PsStatus.ok;
    print("model: %\n",target.origin);
    print("layers: % linear, % full attention, width %\n",target.shape.state.linear_layers,target.shape.state.full_layers,target.shape.linear.width);
    print("vocabulary: %, tensors: %, shards: %\n",target.vocab,target.tensors,target.shards);
    plan:=ps_plan(1,86,86,49);
    #assert plan.status==PsPlanStatus.ok;
    print("shard files: % bytes\n",plan.weight_bytes);
    print("86-token request: % state floats, % workspace floats\n",plan.state,plan.work);
}
```

Target 1 pins the `baselabs/mammoth-1` revision, its config, tokenizer,
readout and shard index hashes, and the size and hash of each shard. The
shape is the one in the pinned config: width 5,120, feed-forward width
17,408, 24 query heads and 4 key/value heads in the full-attention layers, and
48 value heads in the linear layers. Elephant has 32 value heads, so each
Mammoth linear layer carries half again as much recurrent state.

## Plan the storage

Chapter 14's harness sized its buffers by hand. With a second model, that
arithmetic belongs in the library. `ps_plan` takes a target, a capacity and
the request's token and prefix counts, and returns every count the caller
must supply:

<!-- source: psionic/lib/decision-plan.coax plan -->
```jai
ps_plan :: (kind:u8,capacity:int,tokens:int,prefix:int)->PsPlan #export_c {
    target:=ps_target(kind,capacity);
    if target.status!=PsStatus.ok {return PsPlan.{status=PsPlanStatus.target,cause=cast(int)target.status};}
    if tokens<1 || tokens>capacity || prefix<0 || prefix>=tokens {return PsPlan.{status=PsPlanStatus.request};}
    s:=target.shape;linear:=pma_params(s,false);full:=pma_params(s,true);
    plan:=PsPlan.{tensors=target.tensors,layers=s.state.linear_layers+s.state.full_layers,layer_work=linear+full,norm=s.linear.width,state=qp_total(s.state)};
    plan.params=s.state.linear_layers*linear+s.state.full_layers*full;
    plan.work=qs_layout(QxModel.{shape=s,vocab=target.vocab},tokens,prefix).total;
    for i:0..<target.shards {plan.weight_bytes+=target.shard[i].bytes;}
    return plan;
}
```

The plan uses the same functions that `ps_open` and `ps_answer` use to check
their inputs, so a plan and an admission cannot disagree. It refuses what the
request would refuse: an unknown target, a capacity outside 1 to 16,384, an
empty request, more tokens than capacity, or a prefix that leaves no suffix.

For the 86-token sample, Mammoth needs 2,640,384 decoded parameter floats,
42,041,344 state floats and 218,486,272 workspace floats, about 1 GiB
together. Elephant needs 14,581,760 state floats and 79,134,080 workspace
floats for the same request. These are the caller's allocations. They do not
include the 54.7 GB of mapped checkpoint bytes, which the operating system
pages in as the layers read them. A plan is arithmetic: it does not check that
the host has the memory, and it is not a resource scheduler.

## Open both shards

Mammoth's weights are split across `model-00001-of-00002.safetensors` and
`model-00002-of-00002.safetensors`, with `model.safetensors.index.json`
naming the shard of every tensor. The session from chapter 14 already takes a
list of files and an index. For target 1 it checks the config, tokenizer,
readout and index hashes and both shard hashes, admits all 1,184 tensors
against the [trusted inventory](../fixtures/models/mammoth/manifest.json),
including the vision tensors that the text path never reads, and requires the
index to agree with the shard headers by name. Only then does it bind the 64
layers. As before, matrices borrow the original bytes and the files must stay
mapped and unchanged while the session is in use.

The decision itself is unchanged. `ps_prepare` renders and tokenizes the same
typed request, and `ps_answer` runs the prefix and suffix blocks and reads the
pinned slots `A` and `B` from the full vocabulary. Mammoth and Elephant pin the
same `tokenizer.json`, so the sample prepares to the same 86 tokens: a
49-token shared prefix and a 37-token suffix.

## Try the original files

Obtain the pinned `baselabs/mammoth-1` revision described in the
[original bundle record](../docs/original-bundles.md) and put its `config.json`,
`tokenizer.json`, `readout.json`, `model.safetensors.index.json` and both shards
in one directory. No weights are in this repository, and the harness does not
download anything. It compiles an owned C host, sizes every buffer with
`ps_plan`, and runs the Coax session, preparation and answer path:

```shell
coax/build.sh
psionic/tests/run_mammoth.sh /path/to/mammoth-directory
```

Hashing 54.7 GB and reading every weight row on one CPU thread is slow. On the
tested host, opening the session took about four minutes and the answer about
two and a half hours; the process peaked at about 51 GiB resident, nearly all
of it mapped checkpoint pages.

The harness checks that the prepared tokens are the pinned stream, then
compares the answer with an independent reference. The model answered `yes`
with probability about 0.944612 and slot mass about 0.472741; the reference
agrees within the 0.0005 tolerance fixed for chapter 14, with differences
below 0.0000003. Mammoth is more confident than Elephant, which
chose `yes` with probability about 0.757 for the same question.

## How the reference was produced

Chapter 14 compared against the whole model loaded by Transformers in
float32. For Mammoth that copy alone is about 108 GB, which this 125 GB host
cannot hold beside the mapped checkpoint. The Mammoth reference instead builds
one Transformers decoder layer at a time, loads that layer's BF16 tensors as
float32, runs all 86 tokens through it with eager attention and the
two-block mask, and frees it. The arithmetic of each layer is the same as in a
whole-model run; only the residency differs. Run on Elephant, this layer-streamed
reference matched chapter 14's whole-model reference within 0.0000003. See the
[verification record](../../docs/verification/psionic-ch11-2026-10-29.md) for
identities, numbers, time and memory.

## What the checks establish

The plan's checks pin both targets' counts and every refusal, and mutation
leaves no survivors. Session checks now also refuse Mammoth requests with the
wrong number of files or tensors. The original files are authenticated in
full, and one typed request through all 64 layers agrees with an independent
float32 calculation. That applies to the recorded revision, request and CPU
profile. It does not establish accuracy on other requests, Mammoth's
[chapter 16](ch016-are-the-probabilities-any-good.md) quality, image input, or a speed anyone would serve at: this is the CPU
reference path.
