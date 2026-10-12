# Clef batching: decisions per second and the real request limits

How many decisions per second a Clef lane answers as questions are batched
into one request, what sets that ceiling, and which of the served request
limits are the model's and which are this server's own guards. Issue
[#11270](https://github.com/OpenAgentsInc/openagents/issues/11270) family;
the base plan is [clef-native.md](clef-native.md).

A **decision** is one answered question. One request with 64 `noul`
questions that answers all 64 is 64 decisions.

## Headline

- **Batching is the whole game.** On this machine (Apple M5 Max, Metal), one
  question per request answers **2.6 decisions/s** at a 500-token prompt.
  The same work batched **26 questions to a request answers 10.5/s** — a
  **4×** gain, and no more than one request is ever in flight.
- **The ceiling here is about 10.5 decisions/s**, and it is flat across
  prompt sizes from 300 to 1,000 tokens. Past roughly 26 questions a
  request, adding questions stops buying throughput: the request takes
  proportionally longer.
- **The 64-question limit is this server's guard, not the model's.** Cloudflare's
  `Clef-Flash` reference scores every option of every question in one forward
  pass and imposes no such cap. The number is a `ClefLimits` default; the
  only grounded limit is the 16,384-token budget, which is the reference
  encoder's own default `max_length`.

## The measured numbers

Every request is cold: a fresh nonce leads its state, so nothing is served
from Psionic's exact-logit cache. The prompt sizes are the harness's
nominal state sizes; the measured token counts are in the raw JSONL.

⚠️ **The Mac was not idle.** Load averaged 50–85 through the runs (a dozen
`coder-new` agents at about 60 % CPU each). The prior Mac audit measured a
quiet machine at roughly half these latencies. Treat the *shape* below as
the finding and the absolute latency as pessimistic.

### Decisions per second, by questions per request

Metal lane, `127.0.0.1:18098`, 25 s a point, zero failed requests in every
cell.

| State asked | q=1 | q=4 | q=13 | q=26 | q=64 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 300 tokens | 2.62 | 7.38 | 10.14 | **10.52** | 10.19 |
| 1,000 tokens | 1.27 | 3.96 | 7.67 | 9.88 | **10.35** |
| 4,000 tokens | 0.36 | 1.29 | 3.52 | 5.58 | 7.65 |

The same shape at smaller states, where the fixed cost is a bigger share:

| State asked | q=13 | q=26 | q=64 |
| ---: | ---: | ---: | ---: |
| 100 tokens | 8.41 | 8.93 | 9.29 |
| 150 tokens | 8.98 | 9.72 | — |

Two readings:

- **At every size the curve flattens near 10/s.** At 300 tokens it peaks at
  q=26 and slips slightly at q=64; at 1,000 it is still rising at q=64. The
  knee sits around 13–26 questions a request.
- **A larger state lowers the ceiling.** The 4,000-token state reaches only
  7.65/s at q=64, because the state's own tokens are paid on every request
  and they do not shrink as questions are added.

### Where the time goes

One warm request per row, from `psionic.timing`. Prefill dominates; the head
is small and **amortizes**: its per-decision cost falls from 46 ms at q=1 to
4 ms at q=64.

| Questions | Prompt tokens | prefill | head | head per decision |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 880 | 461 ms | 46 ms | 46 ms |
| 4 | 1,205 | 812 ms | 84 ms | 21 ms |
| 13 | 2,135 | 2,099 ms | 102 ms | 7.9 ms |
| 26 | 3,745 | 2,848 ms | 155 ms | 6.0 ms |
| 64 | 8,856 | 6,942 ms | 268 ms | 4.2 ms |

## Why the ceiling is where it is

A request's cost is roughly its prefill, and prefill runs at a steady rate
on this lane:

- **Prefill throughput** is 1,450–1,650 tokens/s across the batched points,
  nearly independent of question count.
- **Each question costs about 120–130 prompt tokens**: at 300 tokens,
  q=1 is 494 tokens and q=26 is 3,649, so 25 more questions cost 3,155
  tokens, or 126 each. Its option texts are the tokens.

So decisions per second approaches `prefill_rate / tokens_per_question`, or
about 1,550 / 128 ≈ **12/s**. The measured 10.5/s is that minus the head and
attention overhead that grow with question count. This is why the ceiling is
**flat across state sizes**: a bigger state lowers the *rate*
`decisions / second` only through its own fixed token cost, not through the
per-question term.

Two consequences:

- **Batching amortizes the state.** Ten files judged one per request pay the
  issue text ten times; batched, once.
- **The ceiling is a token-rate limit, not a compute limit.** A faster
  prefill raises it directly: on an H100 the base shape reached 9.9
  decisions/s unbatched and 39/s batched at 13 questions, against this
  machine's 0.4/s and 3.5/s at 4,000 tokens
  ([decision-throughput.md](decision-throughput.md)).

## Are the limits real or arbitrary?

`ClefLimits` (`crates/psionic/crates/psionic-serve/src/clef/mod.rs`) holds
the served limits. Only one is the model's.

| Limit | Value | Grounded in | Real? |
| --- | ---: | --- | --- |
| `max_tokens` | 16,384 | The reference encoder's own default `max_length` (`encode_record`) | **Yes** — the reference's default, not the architecture's ceiling |
| `max_questions` | 64 | Nothing external | **No** — this server's guard |
| `max_options` | 255 | Nothing external | **No** — this server's guard |
| `max_queue` | 8 | Nothing external | **No** — an operator policy |
| `max_body_bytes` | 8 MiB | HTTP practicality | A practical bound |

### The 16,384-token budget

The one grounded limit. Cloudflare's `joint_schema_model.py` `encode_record`
takes `max_length` with a **default of 16,384**, and Psionic mirrors it as
`TRAINED_LENGTH`. It is not the backbone's ceiling: the shipped
`Clef-Flash` config sets `max_position_embeddings` to **262,144**. So the
16k budget is the reference's serving default, and a caller can raise it on
this server with `--decision-max-tokens` (the H100 runs used `65536`).

### The 64-question and 255-option caps

Both were introduced by the M1 commit (`3168c986aa`) with no stated
derivation, and both appear only as Psionic-side defaults. The reference
imposes neither:

- **The model has no per-question limit.** The model card states Clef "returns
  a probability for every allowed option of every question in a **single
  forward pass**", and the head
  (`crates/psionic/crates/psionic-serve/src/clef/head.rs`) builds its option
  structures with `Vec` growth and batches the question projection — there is
  no fixed array sized to 64 anywhere in the path.
- **255 looks like a `u8` convention**, but the code does not tie it to a
  `u8`; it is a plain `len() > max_options` check.

What they really bound is **cost, not correctness**: every question adds
about 128 prompt tokens and every option adds head work, so a very large
request is slow and memory-hungry, and the guards keep a single request from
monopolizing the lane. They are the kind of limit a server should have —
but they are **this server's policy**, not the model's, and raising them is
a config decision rather than a new capability. In practice a request is
also bounded by its 8 MiB body and by the **32 KiB per-question** bound the
NIP-DEC gateway enforces (the router's `cli_group`, at 33 KB, is refused at
the gateway — see [decision-throughput.md](decision-throughput.md)).

**Finding one's own upper limit.** Measured on this lane, price is flat until
roughly 13–26 questions a request; past that, throughput stops rising and
only latency grows. The useful operating point is the knee, not the cap.

## Applying it: the four-hand solver

`coder solver N` runs an issue to a pull request through four constrained
hands — Intaker, Briefer, Coder, Tester
([solver.md](../coder/solver.md)). A decision model is the natural fit
wherever a hand faces a bounded, typed question rather than free-form code,
and batching changes what is affordable.

### Briefer: file relevance — the clear win

The Briefer's code-brief algorithm
(`crates/coder-new/src/issue_run/decide.rs`, `file_questions`) asks exactly
one question: **"Does the fix for this issue need to change this file?"** It
currently sends **one request per file**, sequentially, each re-sending the
whole issue state:

```rust
for (row, path) in asked.iter().enumerate() {
    let state = format!("{}\n\nFile: {path}\n\n{head}", issue_state(base));
    let request = jev::SystemOneRequest::new(
        state,
        jev::Questions::new().with("relevant", jev::Noul::new(question)),
    );
    // ... one round trip per file
}
```

It judges `(options.files * 2).clamp(6, 12)` files — 6 to 12 round trips for
what is one decision each. Batched into a single request with one `noul` per
file (`f1…fK`), the same work is **one round trip and one prefill** of the
issue text instead of N. At the 1,000-token row above that is roughly the
difference between 12 sequential requests at 1.27 decisions/s (about 9 s)
and one batched request in about 2 s.

Two precedents already in the tree do this batching and can be copied:
`crates/coder/src/codebase.rs` (`questions()` builds `relevant_1…relevant_N`
in one request) and `crates/coder-delegate/src/briefing_jev.rs`
(`entry_{i}` / `requirement_{i}` per item, one request).

**Quality caveat — measure before switching.** The relevance study
([clef-jev-relevance-bench.md](clef-jev-relevance-bench.md), 56 labeled
files) found that batching's effect on accuracy **depends on the model**:

| Judge | F1, one file per request | F1, one request per issue |
| --- | ---: | ---: |
| Jev (hosted) | 0.78 | **0.89** |
| Clef 27B | 0.81 | **0.88** |
| **Clef-Flash** | **0.65** | 0.44 |

Batching *helped* Jev and Clef 27B and *hurt* Clef-Flash in that study (it
scores the option set jointly, and more concurrent options appear to dilute
confidence). So the Briefer's file-relevance step should batch only behind a
re-run of that bench on the door it uses, or keep one-per-request on a
Flash-first door and batch on a Jev or 27B door.

### Briefer / Tester: other bounded judgments

- **Change kind.** `issue_kind` asks a `Choice` ("what kind of change does
  this issue ask for?"). It is one request already; it shares the Briefer's
  door and can ride in the same batched call as the file questions.
- **Tester per-check triage.** When a check fails, "did this failure come
  from the Coder's change, or is it a pre-existing fault at the base
  commit?" is a bounded `noul` per failed check. With a handful of checks
  that is a natural batch, and batching lets the Tester also ask the
  context questions ("is this file where the fix belongs?") in the same
  round trip.

### The swarm: many solvers, one lane

`solver` also runs as a swarm — N issues, N solvers at once
([subagent-channel.md](../coder/subagent-channel.md)). Each Briefer
independently triages its own files. Batched, a swarm's triage load is
`sum(files per issue)` decisions spread over the lane's ~10/s; unbatched it
is the same decisions but at one *state* re-send each, and the lane serves
one request at a time, so concurrency buys queue time, not throughput (see
[decision-throughput.md](decision-throughput.md), "Psionic serves one
decision at a time").

### Where batching does not help

- **The Coder** codes; it does not answer typed questions, so nothing here
  applies.
- **The router** (`crates/coder/src/router/judge.rs`) already batches: the
  main request holds every question, and only questions a rule reads go in
  side requests, sent at the same time. It is the model for the change the
  Briefer needs.
- **Free-form judgments** — "is this analysis good enough?" — are not
  batches of typed questions and stay where they are.

## What to change

1. **Batch the Briefer's `file_questions`** into one request with one `noul`
   per candidate file, following `codebase.rs`. Behind a re-run of the
   relevance bench on the Briefer's door, because batching's quality effect
   is model-specific.
2. **Raise the knee, not the cap.** Tune the questions-per-request toward
   the 13–26 knee; going past it costs latency for no throughput.
3. **Raise `max_questions`/`max_options` only with a reason.** They are
   guards, safe to lift for a known workload, but the token budget and the
   NIP-DEC per-question bound are the limits that actually bind.

## Reproduce

```sh
H=scripts/bench/decision-throughput
# fetch the corpus (the harness reads a directory with dataset.json and router.jsonl)
python3 -I $H/harness.py --target mac-clef --base http://127.0.0.1:18098 \
    --plan full --corpus DIR --out sizes.jsonl --duration 45
# the batch sweep: questions per request against prompt size
python3 -I $H/batch_sweep.py --corpus DIR --sizes 300,1000,4000 --qs 1,4,13,26,64 \
    --duration 25 --out batch.jsonl
# one probe per question count, to find the served cap
python3 -I $H/batch_sweep.py --corpus DIR --out /dev/null --probe --qs 1,64,65
```

`batch_sweep.py` reuses the harness's own request builder and HTTP caller.
The Mac numbers here were taken with `psionic-openai-server -m
Clef-Flash-Q4_K_M.gguf --decision-device metal` on a shared machine; run on
an idle Mac for gate-grade latency. There is no GPU sampler for Metal.
