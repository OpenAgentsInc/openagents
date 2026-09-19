# Compiled functions: settled enough to act on

**Status:** settled on what it is; **one experiment left**, and it is the
cheapest thing on this page's whole reading list. ProgramAsWeights is real,
technically interesting, and unusually honest about its own limits. It does
not serve our contract. But it offers a one-hour, zero-dollar test of
whether our selection guide is wrong at the root, and we should run it.

## What it actually is

A **hypernetwork**. Not an LLM writing a training config, not retrieval over
pre-trained adapters. A trained 4B Qwen3 model takes a spec, runs **one
forward pass**, and emits mixing coefficients over a shared learnable basis
of 64 LoRA bases per module type. Rank 64, all seven projections, all layers,
about 38.5M parameters injected into a frozen Qwen3-0.6B interpreter.

Alongside it, an untrained 4B model restates the spec as a "pseudo-program"
that is **prepended to every call at run time**. Worth noting: a compiled
function is not prompt-free at inference. It is a LoRA *and* a permanent
200–500 token few-shot prompt inside a 2048-token context.

The artifact is a ZIP holding `meta.json`, `adapter.gguf`, and
`prompt_template.txt`, about 22 MB, with the base model pinned by sha256 in
the client. That is comparable to an `.fmadapter` package for inspectability
and arguably better on base integrity. The asymmetry: **you cannot rebuild
it.** Compilation is a hosted service with no self-host path and no published
pricing.

## The labels question, which is the only one that mattered

This page was opened because a compiler that turns English into a working
adapter *without labels* would make [`../choosing.md`](../choosing.md) wrong
at the root.

**The API takes one string.** The entire compile request body is
`{"spec": spec, "public": public}`. So mechanically the claim is true.

**Their own guidance immediately undoes it.** The hosted `AGENTS.md` leads
its spec-writing section with:

> **The #1 practice: iterate with test cases.** Do not accept low performance
> on the first try. Build a test suite of input/output pairs, measure
> accuracy, then iteratively adjust wording.

and

> **Include examples from your actual data**: Examples outperform prose-only
> descriptions.

Every worked example in their docs embeds three or four input/output pairs
inside the spec. So labels return at once — first as few-shot examples in the
prompt, then as the test suite you iterate against. It is not labels versus
no labels. It is **labels as a prompt rather than labels as a gradient**.

**And the accurate tier manufactures them.** The second paper's compiler has
GPT teachers generate **3,600 unique synthetic pairs**, replicated to 4,800,
then runs 100 steps of ordinary LoRA fine-tuning warm-started from the
hypernetwork's guess. That is not "no labels." That is a frontier model
writing your labels.

### So the root survives, and the reason is worth keeping

PAW does not remove "do you have labelled outcomes?" It adds a **third
branch**: *you have no labels and will accept a frontier model's guess at
what your labels would be.*

For a decision model whose labels encode a real outcome — which door actually
worked, which ticket the customer was actually routed to — a teacher LLM
cannot synthesize that. It can only synthesize its own prior. **Our 98
records contain information gpt-5.5 does not have**, and that is the entire
value of a labelled outcome.

## It does not serve the contract

The **third** system reviewed this week presented as a general decision
substrate while serving Choice alone. To its credit, PAW does not claim
otherwise — it claims to be a fuzzy-function compiler, and Choice is an
honest fit for that claim.

Every function is `str -> str`. A live call returns:

```json
{"output":"immediate","tokens_generated":3,"latency_ms":75.3}
```

A label. No option set in, no distribution out.

- **Noul:** not served. There is no probability output at all.
- **Score:** served only as a relabelled Choice, and **their own case study
  documents the failure**. Attempt one was numeric 1–10 relevance scoring:
  *"The model clustered everything at 8–10 [...] No discrimination."* Their
  conclusion: *"Small models can't produce fine-grained numeric scores. They
  don't have a calibrated sense of what 7 vs 8 means."* What worked was four
  named categories mapped to integers **in Python**. The ordering lives in
  the caller, not the model.

**Calibration is absent, not merely weak.** No ECE, no Brier, no reliability
diagram in either paper; the word "calibrated" appears once, describing
someone else's work. It does not clear the bar we set this week — it does not
even produce numbers that sum to one.

## The benchmark is not the comparison we needed

| Method | FuzzyBench exact match |
| --- | --- |
| gpt-oss-20B, prompted, local | **85.45%** |
| **PAW on Qwen3-0.6B** | **73.78%** |
| Qwen3-32B, prompted | 68.70% |
| Qwen3-0.6B, prompted, same base | 9.84% |

The abstract says PAW "matches direct prompting of Qwen3-32B," which is true
and a carefully chosen comparator — **an open-weights model you could run
locally beats it by 11.7 points, in the same table.**

And the fine-tuning comparison is not per-task. PAW beating "full fine-tuning
by 15.4 points" is against a single *multi-task* adapter trained on 10M
examples and asked to handle **held-out specs unseen at training**. The paper
says so itself. **It never compares a compiled adapter against ordinary
fine-tuning on the target task with that task's own labels** — which is
exactly the comparison our week sits on the other side of.

The one independent replication is the most useful number found. An outside
evaluator, 100 adversarial cases on a real classifier:

| Variant | Accuracy | Median latency |
| --- | --- | --- |
| hypernetwork | 94% | 41 ms |
| teacher-synthesis fine-tune | 97% | 38 ms |

Their own reading: *"within noise at n=100 (McNemar p=0.51), so we do not
claim finetuned beats standard."* **Our 0.056 floor would swallow that gap
entirely.** The expensive path did not measurably beat the five-second one.

Two claims that do not survive checking: *"compiler, interpreter, training
pipeline, and web app are all open source"* — the training pipeline and web
app are not in the public org, and the compiler weights carry no licence
field. And *"runs on a CPU"* — **the word CPU does not appear in the paper at
all**; every latency is Metal-accelerated Apple silicon, and the SDK frames
CPU as the fallback for when GPU breaks.

## Two things to borrow now, no dependency required

**Semantic abstention beats confidence thresholding.** The independent
evaluator found PAW adapters *confidently wrong* at 0.97–1.00 on their
misses — thresholding on confidence failed outright. Adding an explicit
`unsure` class to the spec worked: 98.9% on decided cases.

That is a cheap, testable hypothesis about our own on-device adapter, and it
bears directly on
[#9383](https://github.com/OpenAgentsInc/openagents/issues/9383): it is
evidence that abstention wants to be *an option the model can name* rather
than a threshold the caller applies afterwards. It is also the same failure
mode we measured — our own adapter got more confidently wrong as it got more
accurate.

**Ordered rubrics want named buckets, not numbers.** Their Attempt 1 → 3
sequence is free evidence for a question we will face if Score is ever served
by a small on-device model: a 0.6B model cannot hold a 1–10 ordinal but can
hold four semantically anchored ordered categories. That predicts a rubric
granularity ceiling, and it connects to
[#9378](https://github.com/OpenAgentsInc/openagents/issues/9378).

## The experiment, which we should run

One hour, zero dollars, no new harness, restricted to Choice so no contract
work is needed.

1. Take the `routing` family, where we already have labelled data.
2. Compile it twice on the free tier: once from **prose alone, zero labels**,
   and once with four example pairs inline.
3. Run both through the existing suite and gate against two baselines we
   already have: our four-minute on-device LoRA trained on **98 real
   labels**, and the on-device base prompted zero-shot.

**The read is unambiguous in either direction**, which is what makes it worth
doing:

- **If the zero-label compile lands within 0.056 of our 98-record LoRA**,
  then our 98 labels bought nothing a 4B hypernetwork could not infer from
  prose, and `choosing.md` needs its third branch.
- **If it does not**, we have a measured number for what real labelled
  outcomes are worth on our task — which is precisely the quantity the PAW
  paper never measures, and the guide's root stands with evidence under it.

Do **not** run the teacher-synthesis tier yet. It needs a 40 GB accelerator
and 3,600 frontier-model generations at an undisclosed cost, and the only
independent measurement says the expensive path did not beat the cheap one
outside noise.

## The part worth agreeing with

> I don't see why we are turning a classifier into another private API.

That is the argument this directory makes for Lev and for Kev, and it is why
there are three doors behind one contract. On the local-and-private axis a
shared small base you control is **strictly better than Lev**: you can read
its distribution directly, where Apple's runtime returns no logits and forces
us to count samples at `1/N` resolution.

The irony is that PAW does not expose that distribution either. The advantage
is available and unused.
