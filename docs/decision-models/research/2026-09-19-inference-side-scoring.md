# Inference-side scoring: settled, and not the way the claim says

**Status:** settled. The mechanism is real and unremarkable; the quality
claim is contradicted by the only public paired measurement of exactly this
architecture. Two things are worth taking from it, and neither is the door.

## The claim

> sglang offers a scoring endpoint in addition to the normal generation one.
> In scoring mode, given an input and a set of possible answers, it forces
> the model to produce probabilities for each one. [...] For deepseek, you
> have to add a closing think tag before the response. [...] dsv4.1 flash is
> not as good as jev, but if we had enough spare compute to experiment with
> this same approach for a larger model then i think the decision quality
> would be at least as good, if not better.

## What `/v1/score` actually computes

Read from the local clone at `projects/repos/sglang` (`320bdd1ee2`), route at
`python/sglang/srt/entrypoints/http_server.py:1915`.

It is a thin wrapper over *prefill, do not generate, read the next-token
logprobs for a list of token ids, optionally softmax them*. The whole
computation is nine lines in
`python/sglang/srt/managers/tokenizer_manager_score_mixin.py:642`.

Three corrections to the usual description:

- **The candidates are single vocabulary token ids**, not strings.
  Multi-token options are not merely unhandled, they are not representable.
  The only validation is `token_id >= vocab_size`.
- **`items` are not the answers.** Each item is a context suffix appended to
  the query; the result is `[num_items][num_labels]`. The canonical use is
  reranking.
- **Softmax is within one item**, never across items. The default
  (`apply_softmax: false`) returns `exp(logprob)`, true full-vocab
  probabilities that do not sum to one.

Two hazards worth knowing: a missing label token silently becomes probability
`0.0` rather than an error, and `token_ids_logprob` requests crash the
scheduler when co-batched with requests that do not use them
([sglang#34719](https://github.com/sgl-project/sglang/issues/34719), unfixed
across 0.5.14–0.5.17). "Just use `/v1/score`" is not free.

## The claim was already tested, publicly, and lost

[`ekzhang/openjev-sglang`](https://github.com/ekzhang/openjev-sglang) is a
complete implementation of `POST /v1/systemone` on Qwen3.6-35B-A3B over
sglang, with a paired evaluation against hosted Jev. It is exactly the door
the post proposes.

On a matched 1,000-question MMLU-Pro subset: **58.8% against Jev's 82.9%.**
A second open model, Qwen3.8-27B, scored 60.0%.

On 3,270 BoolQ examples, where it does much better:

| | OpenJev / Qwen3.6 | hosted Jev |
| --- | --- | --- |
| Accuracy | 89.45% | **91.56%** |
| Brier | 0.0812 | **0.0640** |
| Log loss | 0.2926 | **0.2257** |
| Mean probability − accuracy | **+3.74%** (overconfident) | −1.87% |
| Selected-answer ECE, 10 bins | 4.05% | **2.51%** |

The tail is the sharper result. Among answers claimed at 99% or above,
OpenJev was wrong 32 times in 1,610; Jev was wrong **once in 366**. That is
the band an application routes on.

A paired bootstrap clustered by passage puts Jev ahead on accuracy, Brier,
log loss, and overconfidence with intervals excluding zero — while the **ECE
interval includes zero**. The honest reading is worse skill and worse
overconfidence with comparable binned ECE, not "worse at everything."

The post's own hedge is the part the evidence contradicts hardest. It
supposes a larger model would close the gap; DeepSeek-V4.1-Flash is a 552B
mixture of experts, already far larger than either model tested, and scale is
not the axis the gap lies on. `openjev`'s own research note reaches the same
conclusion from inside the implementation: closing it is *"a training
project, not a serving flag or a small prompt fix."*

## The think tag is documented behaviour, and it is not free

Mechanically it closes an empty reasoning block so the next-token
distribution is over the answer rather than over the first token of a chain
of thought. It is not a trick — DeepSeek documents it, and on their hosted
API it is a request field rather than a string hack.

What the post does not mention is the cost. From `openjev`'s held-out
measurement, 128 MMLU-Pro questions, same prompt, reasoning budget varied:

| Reasoning cap | Accuracy | Median latency |
| --- | --- | --- |
| 0 tokens (the trick) | 53.9% | **0.11 s** |
| 64 | 51.6% | 0.61 s |
| 256 | 53.1% | 1.89 s |
| 1,024 | **68.8%** | 7.25 s |

Closing the think tag forfeits about fifteen accuracy points on
reasoning-heavy choice to buy a 65-fold latency reduction. That is the actual
trade. Note also that small budgets are *worse* than none: a truncated
calculation is worse than no calculation.

## `jev-recruiter` is not evidence for any of this

It never touches sglang, `/v1/score`, logprobs, or a think tag. It is a Jev
*client* — one `POST /v1/systemone` call per decision, every question
`type: "choice"`, no `noul` and no `score` anywhere in the repository. Its
only DeepSeek reference is an ordinary `chat/completions` text-fill helper
the README says is not exposed.

Its own README is more careful than the posts around it:

> A `potential_match` means Jev marked every required criterion as met and
> supplied observed quotations. It is **not an independently verified
> qualification or a hiring decision** [...] A quotation can be real while
> the model's interpretation is wrong.

It samples at most seven screens per profile, messages nobody, and leaves
shortlisting to a person. The claim that decision models eliminate sourcing
work is the marketing around the artifact, not the artifact.

## The finding worth keeping: sglang has independently built Kev's mask

This is what the investigation actually bought.

With `--enable-mis`, sglang packs a query and many items into one sequence
and reads label logprobs at each delimiter — *"score each option's closing
delimiter position against nominated tokens"*, which is Kev's readout
position with a frozen LM head in place of a trained pointer head. And its
FlashInfer mask
(`python/sglang/srt/layers/attention/flashinfer_backend.py:564`) gives each
item the shared prefix and itself and nothing else, which is Kev's
`option_isolation` variant.

The difference matters: **for Kev that isolation is a flag and it is off by
default**, because with it off the decision token sits after every option and
reads the whole list — which is what makes "none of the above" and listwise
effects work. sglang hard-wires the isolated form, enforcing independence of
irrelevant alternatives by construction, which is precisely the behaviour
Hume measured hosted Jev *violating*.

So the correct statement of the mechanistic difference is not "independent
against listwise" — `openjev` renders all options into the prompt and reads
one softmax over label tokens at a shared position, which is listwise. It is
**a frozen language-model head over nominated vocabulary tokens, against a
head trained for the decision.** Both can be listwise; only one was trained
to be calibrated.

## Score is the untested primitive, everywhere

Noul and Score both fall out of scoring in about forty lines, and `openjev`'s
Score formula is `Σ i · p_i` — character for character the same as Kev's,
arrived at independently, which is mild evidence it is the right reading of
the contract.

But **nothing measures Score, in any public artifact found**. BoolQ tests
Noul, MMLU-Pro tests Choice, Score is untested everywhere. And it is exactly
where a label-token readout is weakest: the ordering has to mean something
for a weighted mean to be meaningful, and `A`, `B`, `C` are arbitrary
vocabulary tokens carrying no ordinal relation. A model can put mass on
levels 0 and 4 with a trough at 2 and `Σ i · p` will report 2. Kev's upstream
training has an explicit ordinal loss term for this; a scoring endpoint has
no equivalent.

We should hold our own Score numbers to the same complaint.

**Done, on 2026-09-19:**
[`../2026-09-19-score-ordinality.md`](../2026-09-19-score-ordinality.md).
Hosted Jev and Lev survive it. `kev-0.5b` does not: 26 of 60 ramp
distributions are bimodal, and on 47 of 96 items the weighted mean is not the
level the model picked.

## What to do

**Do not build a fourth door.** It exists, it is better than we would build
in a week, and it measured itself into a 24-point hole on MMLU-Pro. Building
our own would re-derive that result at our expense. `openjev-sglang` also has
**no licence file**, so it is read-and-cite only regardless.

**Take the evaluation harness instead — it is better than ours.** 3,270
examples with a passage-clustered bootstrap, paired difference intervals,
resumable collection that refuses to report on missing rows, and dataset
byte-hash pinning. Our ECE is a fixed ten-bin table with no bootstrap and no
clustering, and Kev's calibration numbers are a replayed upstream fixture
with no harness at all. This is directly relevant to
[#9376](https://github.com/OpenAgentsInc/openagents/issues/9376), which needs
a defensible spread for the calibration metrics and would otherwise invent
one.

**Keep the mask as a reference, not as code.** When Kev's `option_isolation`
serving mode is built, sglang's FlashInfer backend is the reference for
expressing "each span attends to prefix and itself" to a batched kernel, and
PR #10979's accuracy check — multi-item scores must equal single-item scores
— is the conformance test to mirror. Borrow the kernel shape, not the
default.

**One half-day experiment is worth running.** We cannot currently say how
much of Jev's edge is the trained readout and how much is the training data.
A one-token readout on the same frozen Qwen backbone Kev already uses,
prompted listwise, scored on the same suite as Kev's pointer head, isolates
that. It needs no new door and no GPU.

## Sources

[sglang native API](https://docs.sglang.io/docs/basic_usage/native_api) ·
[sglang PR #10979](https://github.com/sgl-project/sglang/pull/10979) ·
[sglang issue #34719](https://github.com/sgl-project/sglang/issues/34719) ·
[ekzhang/openjev-sglang](https://github.com/ekzhang/openjev-sglang) ·
[skeptrunedev/jev-recruiter](https://github.com/skeptrunedev/jev-recruiter) ·
[DeepSeek-V4.1-Flash encoding reference](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash/blob/main/encoding/README.md)
