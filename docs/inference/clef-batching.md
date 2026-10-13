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
- **The 64-question limit is not the model's.** Cloudflare's `Clef-Flash`
  reference scores every option of every question in one forward pass and
  imposes no such cap; the backbone context is 262,144 positions. The 64 is
  **our** bound — the NIP-DEC protocol's (`crates/nostr/src/decision.rs`
  `MAX_QUESTIONS`), which the Clef server took. The only other limit is the
  16,384-token budget, the reference encoder's own default `max_length`.
- **Pack representations, not files.** A candidate file can be thousands of
  lines; a request holds one `state` of at most 128 KiB inside the 16k budget.
  Send each file as a bounded representation — path and finder reasons, then
  its outline, then code excerpts — from tools this repository already has.

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
the served limits. None is the model's:

| Limit | Value | Grounded in | The model's? |
| --- | ---: | --- | --- |
| `max_tokens` | 16,384 | The reference encoder's own default `max_length` (`encode_record`) | No — the reference's serving default |
| `max_questions` | 64 | **NIP-DEC's protocol bound** (`crates/nostr/src/decision.rs` `MAX_QUESTIONS`) | No — our protocol's bound |
| `max_options` | 255 | **NIP-DEC's protocol bound** (`too_many_options` above 255) | No — our protocol's bound |
| `max_queue` | 8 | Nothing external | No — an operator policy |
| `max_body_bytes` | 8 MiB | HTTP practicality | No — a practical bound |

The model itself imposes none of these. The model card says Clef "returns a
probability for every allowed option of every question in a **single forward
pass**", and the head
(`crates/psionic/crates/psionic-serve/src/clef/head.rs`) builds its option
structures with `Vec` growth and batches the question projection — there is
no fixed array sized to 64 anywhere in the path.

So the caps come in two kinds, and it matters which you are hitting:

- **The model's limits:** unbounded questions and options; the backbone
  context is 262,144 positions. The 16,384-token budget is the *reference
  encoder's default* (`joint_schema_model.py` `encode_record`), which
  Psionic mirrors as `TRAINED_LENGTH`; a caller can raise it here with
  `--decision-max-tokens` (the H100 runs used `65536`).
- **Our limits:** the 64-question and 255-option caps are **the NIP-DEC
  protocol's bounds**, not only this server's — `crates/nostr/src/decision.rs`
  exports `MAX_QUESTIONS = 64` and refuses `too_many_questions` /
  `too_many_options`, and the M1 server took the same numbers. They are a
  deliberate, uniform bound on every host and door, not an accident of the
  Clef server. The 128 KiB `state` bound and the 64 KiB per-question bound
  are NIP-DEC's too (`nips/openagents/NIP-DEC.md`).

What the caps really bound is **cost, not correctness**: every question adds
about 128 prompt tokens and every option adds head work, so a very large
request is slow and memory-hungry, and the bounds keep one request from
monopolizing the lane. Raising them is a protocol change (bump `MAX_QUESTIONS`
in `crates/nostr` and every host's config), not a server flag.

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

## How to pack many files into one request

The hard part of batching file relevance is **what to send per file**. A
candidate file can be thousands of lines, and a request holds one `state` of
at most 128 KiB (`nips/openagents/NIP-DEC.md`) inside a 16,384-token budget,
so the whole file does not fit for more than a few candidates. The field
packs one **representation per file**, not the file, and never re-sends the
issue text.

Send the candidates as one `state` object, one array entry per file, with an
increasing `id` (`f1…fK`), and one `noul` question per entry that names that
id:

```json
{
  "model": "clef-flash",
  "state": {
    "issue": {"number": 11247, "title": "Only what a rule reads", "body": "…"},
    "candidates": [
      {"id": "f1", "path": "crates/coder/src/router/judge.rs",
       "finder": {"rank": 1, "confidence": 0.71},
       "reasons": ["issue names `route`", "defines `route_instructions`"],
       "outline": ["120 fn route_instructions(", "…"],
       "excerpts": ["--- lines 120-160 of 2724 ---", "…"]}
    ]
  },
  "questions": {
    "f1": {"type": "noul", "instructions": "Is file f1 (path above) relevant to solving the issue?"},
    "f2": {"type": "noul", "instructions": "Is file f2 (path above) relevant to solving the issue?"}
  }
}
```

### The four tiers of per-file detail

Every tier below was sent as **one** request against the live Metal lane, with
six real candidate files from this tree (a router issue's likely set:
`router/judge.rs`, `first.rs`, `cli_route.rs`, `select.rs`, `codebase.rs`,
`jev/src/lib.rs`). The `prompt_tokens` and times are what the server reported.

| Tier | Per file, in the request | Total state | Prompt tokens | prefill | head |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | path, finder rank, confidence | 686 chars | 801 | 474 ms | 143 ms |
| 2 | + finder reasons | 1,097 chars | 922 | 458 ms | 52 ms |
| 3 | + **outline** (declaration lines) | 16,628 chars | 6,356 | 5.15 s | 263 ms |
| 4 | + **excerpts** (code windows) | 34,534 chars | 11,728 | 9.45 s | 188 ms |

Read it as a menu, and pick by how much the answer needs the code:

- **Tier 1–2 are nearly free** — the whole candidate set for about 1,000
  tokens, under half a second. This is enough for a *pruning* pass: the path
  and the finder's reasons ("issue names `route`", "defines
  `route_instructions`") are strong signals for a first cut. This is exactly
  the batched-relevance shape the bench already asks, and the cheapest.
- **Tier 3 adds the outline** — the file's declarations, one line each,
  without their bodies. It costs ~6,400 tokens for six files (about 1,000
  tokens a file) and tells the model *what the file defines*, which is what
  "does the fix need this file?" usually turns on. For 12 files it is about
  8,800 tokens (measured by rendering all 12 outlines), still inside the
  budget.
- **Tier 4 adds excerpts** — a window around each matched line. It is the
  most expensive (~11,700 tokens for six files) and will not scale past
  roughly 8 files inside 16k. Reserve it for the top few after a cheaper pass
  dropped the rest, or raise `--decision-max-tokens`.

If a call does exceed the budget, NIP-DEC leaves a way through: `truncation:
"state_tail"` (the `truncation` field) keeps the tail of the state instead of
refusing, and the response reports `truncated_state_tokens` so the caller can
tell.

### Build each tier from tools we already have

Every representation above is already produced by code in this repository, so
none of it needs new parsing:

- **Path, rank, confidence, reasons** — `filefind.py query --json` emits
  exactly this per file (`scripts/filefind/filefind.py`):
  `{"rank", "path", "confidence", "stages", "reasons"}`. The reasons are the
  finder's own, from its nine stages ("issue names `tok`", "defines
  `route_instructions`", "changes with the seeds (co 0.58)",
  "fixed with similar #N (sim 0.71)").
- **Outline** — the briefing generator already renders one. `briefing.py
  outline()` (`scripts/bench/briefed-ab/briefing.py`) matches the declaration
  lines of a Rust file and prints `line  <source>`; the terminal briefing
  path uses it for any file the finder listed without a text match. Its own
  header line, `--- outline: N items of M lines ---`, goes straight into the
  request. Example, the first items of a real file:

  ```
  --- outline: 60 items of 1038 lines ---
     47  pub const SET: &str = "codebase-kb-v1";
     50  pub const INDEX_VAR: &str = "CODER_CODEBASE_KB";
    101  pub fn configured_path() -> Option<PathBuf> {
    228  pub fn state(question: &str, candidates: &[&Chunk]) -> Value {
    250  pub fn questions(candidates: usize) -> Questions {
  ```

  The structured form is `briefing-lab/src/syntax.rs` (`extract`), which
  returns each declaration's `kind`, `name`, `qualified_name`, `signature`
  span and body span from tree-sitter — so an outline can carry signatures
  and line ranges rather than raw lines.
- **Excerpts** — again `briefing.py excerpt()`: it opens a window (`ln-12`
  to `ln+18`) around each matched line, merges overlapping windows, and
  prints `--- lines LO-HI of N ---`. This is the same excerpt the briefing
  already hands the Coder. Example:

  ```
  --- lines 228-267 of 1038 ---
    228  pub fn state(question: &str, candidates: &[&Chunk]) -> Value {
    237          "text": chunk.text.chars().take(JUDGE_CHARS).collect::<String>(),
    250  pub fn questions(candidates: usize) -> Questions {
  ```
- **A per-item text cap** — `crates/coder/src/codebase.rs` already bounds a
  candidate's text to `JUDGE_CHARS = 1_200` (`chunk.text.chars().take(...)`),
  a per-candidate budget of roughly 300–400 tokens. That is the same idea as
  the outline tier: give each file a fair slice, not the whole file.
- **A whole-repository summary** for a *relevance* call — `plugin-repo-map`
  (`crates/plugin-repo-map/src/lib.rs`) returns the repository's shape (file
  counts, languages, top directories, largest files, manifests, test dirs)
  without reading contents. Useful as shared context a candidate set is
  judged against, not per file.

### Packing rule

Full file text does not fit, so **never send it**. Give each file a bounded
representation and a bounded character budget, the way the briefing already
does (`excerpt_lines` caps lines per file; `budget_by_package()` trims the
longest excerpt until the Markdown fits a character budget). For a batched
relevance request the same rule applies to `state`: hold a byte budget, give
each candidate its outline, and drop to tier 1–2 or cut excerpts when the set
grows. The head amortizes (its per-decision cost fell from 46 ms at one
question to 4 ms at 64), so the cost of a bigger batch is prefill tokens —
which is exactly what the outline tier spends fewer of.

### Several passes over one issue: only the issue in `state`

A Psionic lane with prefix reuse on (`PSIONIC_CLEF_PREFIX_CACHE_BYTES` on
Metal; on by default on the H100 lanes, see clef-native.md Round 9) prefills
a state once and answers later requests with the same state from a snapshot.
The reused prefix is the system prompt plus the **whole** rendered `state`
(on CUDA rounded down to a multiple of 16 tokens), matched token for token;
the schema (the questions and options) is computed fresh every time. So:

- **The shared text is the entire `state`.** For a tier-2 pass and a tier-3
  re-ask on one issue, send the issue alone as the state and put each
  candidate's detail in its own question. `instructions` may be a JSON
  object, so the per-file representation moves there unchanged:

  ```json
  "questions": {"f1": {"type": "noul", "instructions": {
      "question": "Does the fix for this issue need to change this file? The file is f1: crates/coder/src/router/judge.rs",
      "file": {"path": "crates/coder/src/router/judge.rs", "finder": {"rank": 1, "confidence": 0.71},
               "reasons": ["issue names `route`"]}}}}
  ```

  `scripts/clef-lab/issue_files.py` `shared_request()` builds this shape.
- **Anything that varies, anywhere in the state, defeats reuse.** A state of
  `{issue, candidates}` matches nothing when the candidates change, even
  though they come last. `render()` sorts object keys, so key order in the
  caller's JSON does not matter, and naming keys so the shared part sorts
  first (`_issue` before `candidates`) does not help either: the snapshot is
  taken only at the end of the state.
- **A nonce belongs to the run, not the request.** To keep the whole-answer
  logit cache out of a measurement, put one nonce per issue run in the state
  (the lab uses `_run`); every pass of that run then shares it.
- **The response says what was reused.** `usage.cached_input_tokens` and
  `psionic.timing.restored_tokens` count the restored prefix tokens; on CUDA,
  `psionic.prefix_cache` reports the snapshots held and their bytes.

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
3. **Send representations, not files.** Pack one bounded representation per
   candidate (path + reasons, then outline, then excerpts) into the single
   `state`, using the tools above. The 128 KiB `state` and 16,384-token
   budget, not the question count, are what bind a real candidate set.

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
# the four per-file representation tiers, as one request each
AB_REPO=. python3 -I $H/file_tiers.py --tiers 1,2,3,4
```

`batch_sweep.py` reuses the harness's own request builder and HTTP caller.
`file_tiers.py` reads the checkout at `AB_REPO` and builds each tier with the
briefing generator's own `outline()` and `excerpt()`, so the state it sends is
the state the tiers table describes.
The Mac numbers here were taken with `psionic-openai-server -m
Clef-Flash-Q4_K_M.gguf --decision-device metal` on a shared machine; run on
an idle Mac for gate-grade latency. There is no GPU sampler for Metal.
