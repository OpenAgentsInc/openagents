# SubQ Snap (`subq-snap-preview`) as a System One door

Status, 2026-10-12: measured on our briefer, router and goldens evals
([#11458](https://github.com/OpenAgentsInc/openagents/issues/11458)). It is
wired as an opt-in door (`--decisions subq`), and it is not the default
anywhere. The demo-style whole-repository curation it makes possible is
written up in [subq-context-curation.md](subq-context-curation.md).

SubQ Snap is SubQ's hosted System One model. `POST
https://api.subq.ai/v1/systemone` takes our body (`{model, state,
questions}`, with noul, choice and score questions and up to 4 images) and
answers with probabilities and confidence, so `crates/jev` speaks it with
no changes, only a different base URL, model and key.

## What the docs say ([docs.subq.ai](https://docs.subq.ai/), read 2026-10-12)

- **Auth.** `Authorization: Bearer <key>`. A key is shown once, when it is
  created at console.subq.ai. Ours is `SUBQ_API_KEY` in
  `~/work/.secrets/subq.env`. `GET /v1/models` checks a key: ours lists only
  `subq-snap-preview`.
- **Models.** `subq-snap-preview` (Decisions, **public preview**, text and
  image input, $0.05 per million input tokens, output free; `usage.output_tokens`
  is always 0). `subq-preview` (chat and Responses, long context, early
  access) is not enabled on our key.
- **Limits.** Up to 4 images (PNG, JPEG or WebP; raw base64 only, no URLs
  or `data:` prefixes), 4 MiB each and 8 MiB in total, about 64 to 256
  tokens per image. The request body may be up to 13 MiB. A choice takes
  2 to 255 options, a score 2 to 10 levels, and question names may be up to
  100 characters. The docs do not state the context window. We sent a
  295k-token state and it was accepted.
- **Rate limits.** No numbers are published. The docs say to retry `429`
  and `503` with backoff, and to fix a `422` rather than retry it. We hit no
  429 in about 900 requests run one at a time.
- **Errors.** `{"detail": {"error_type", "message"}}`, with 401 (credentials),
  402 (no credits), 403 (model not allowed for the key), 422 (invalid
  request, bad image, or not a decision model), 429 and 503.
- **Structured outputs and function calling** exist only for
  `subq-preview` (early access), not for Snap. Snap's typed answers are its
  structured output.
- **Data retention: the default is to keep everything indefinitely.** Each
  organization has one policy: keep indefinitely (the default), delete after
  N days, or zero data retention. ZDR is requested through a form, using the
  sign-in email. Stored decisions include their images. Decisions work the
  same under ZDR, and there is no per-request `store: false` for Decisions.
  Model, token counts, timing and metadata tags are always recorded, but
  never prompts or completions. The docs say nothing about training on
  customer data. **For us:** every state we send (issue text, file bodies,
  chat transcripts) is kept until we delete it, unless the org is switched
  to ZDR first.

## Smoke test

| Request | Result | Latency (this Mac, wall) |
|---|---|---|
| Docs example (noul + choice + score, 260 tokens), 3 runs | escalate 0.95, `nurse_line` 0.96, acuity 1.38 | 368 / 371 / 392 ms |
| Image: a terminal screenshot (404 tokens), 2 runs | is_terminal 0.92, `terminal_ui` (confidence 0.98) | 447 / 391 ms |
| Image: a butterfly sprite sheet (426 tokens), 2 runs | is_terminal 0.01, `butterflies` (confidence 0.98) | 773 / 1,775 ms |
| `crates/jev` client at `api.subq.ai` (`the_subq_door_answers`) | `small` 0.98, 99 tokens | 0.58 s, test included |

Answers were identical across repeats.

## Briefer file judgments (ba1, 21 issues)

These use the same frozen candidates as [briefer-arms.md](briefer-arms.md):
12 per issue, rebuilt from `results/briefer-arms-ba1/judgments.csv`. The
labels are the fix commit's files. Clef and hosted come from ba1. The SubQ
arms are new:

- `subq`: decide.rs's exact question, one Noul per file over the issue and
  the file's first 80 lines. Keep at p ≥ 0.35, keeping at least 3.
- `subq-perfile-full`: the same, but with the whole file (up to 600k
  characters) instead of 80 lines. This separates the effect of the model
  from the effect of the context.
- `subq-full`: SubQ's demo shape. One request per issue holds all 12
  candidates' full bodies in one state (water-filled to 1M characters,
  about 190k tokens on average). The issue is split into narrow questions:
  7 issue-independent roles (main change, cause, test, types, caller,
  user-facing surface, first file a reviewer opens), plus one per paragraph
  or list item of the issue, up to 10. Each is a Choice over the files plus
  "none". A file is kept when any question gives it p ≥ 0.3 (union, no
  minimum).

| Arm | Accuracy | Precision (keep) | Recall in candidates | Fix recall | Top 1 is a fix file | AUC | Kept (mean) | $ per issue | Seconds p50 / p95 |
|---|---|---|---|---|---|---|---|---|---|
| clef (local) | 0.735 | 0.288 | 0.543 | 0.334 | 0.381 | 0.70 | 3.14 | 0 | 11.75 / 13.23 |
| hosted Clef | 0.766 | 0.338 | 0.681 | 0.429 | 0.571 | 0.78 | 3.24 | unpriced | 11.73 / 37.77 |
| **subq** | 0.750 | 0.324 | 0.726 | 0.456 | **0.762** | **0.84** | 3.52 | **0.0011** | 6.19 / 6.78 |
| subq-perfile-full | 0.714 | 0.308 | **0.837** | **0.560** | 0.524 | 0.83 | 4.33 | 0.0104 | 9.77 / 11.14 |
| **subq-full** | **0.825** | **0.434** | 0.707 | 0.456 | 0.571 | 0.84 | 2.52 | 0.0096 | **3.75 / 4.29** |

AUC is how well a file's probability separates fix files from the rest.
For subq-full it is each file's highest probability over the questions.
Of the 37 fix files, the threshold keeps 13 (clef), 10 (hosted),
20 (subq), 25 (perfile-full) and 23 (subq-full).

Haiku comparison: the Haiku arm answered 20 of 21 issues (all but
#10181). On those 20:

| Arm | Accuracy | Fix recall | Top 1 |
|---|---|---|---|
| Haiku (pooled ba1 + rerun) | **0.888** | **~0.74** | **~0.90** |
| subq-full | 0.821 | 0.478 | 0.60 |
| subq | 0.750 | 0.478 | 0.80 |
| hosted Clef | 0.767 | 0.451 | 0.60 |

What this says:

- **On the same question, Snap is the best door we have measured.** It has
  the highest AUC (0.84 against hosted Clef's 0.78), top-1 0.76 against
  0.57, and twice as many fix files clear the 0.35 bar. It is also twice
  as fast as Clef and costs about $0.001 per issue. Accuracy is flat
  (0.75), because accuracy at a fixed keep rule mostly measures the
  `MIN_FILES` top-up.
- **Context, not the model, is what limits the 80-line door.** With whole
  files, recall rises (0.56 fix recall, 0.84 inside the candidates). But
  precision and top-1 fall, because a long file gets a high p more easily
  (it keeps 4.3 files per issue).
- **The demo shape is the most precise door.** subq-full makes one request
  per issue in under 4 s and gives the highest accuracy (0.825) and keep
  precision (0.43) of any door. It does this by keeping fewer files (2.5),
  so its fix recall equals the per-file arm's. Under decide.rs's rule
  instead of the union rule (keep at ≥ 0.35, at least 3), it reads 0.778
  accuracy, 0.354 precision and 0.458 fix recall. The role questions did
  most of the work: `role_main_change` kept 24 files, 17 of them fix
  files. Issue-paragraph questions after the second rarely kept anything.
- **Haiku still wins on judgment**, by 0.07 accuracy, about 0.26 fix
  recall and 0.3 top-1 over the best Snap arm. The Haiku agent can grep
  outside the candidates, and a door cannot. The decision in
  briefer-arms.md stands.

**Line ranges** (the demo's second request; `briefer_arms.py lines`): over
subq-full's kept files, cut into fixed windows (at most 250, at least 40
lines each), the same narrow questions are asked as a Choice over windows.
A window is kept at p ≥ 0.1 on any question.

| Issues | Packet p50 / max | Kept files' size (mean) | Fix hunks covered (in kept files) | Fix hunks covered (all) | $ total |
|---|---|---|---|---|---|
| 21 | **7.9 kB** / 15.6 kB | 155 kB | 39 / 76 (51 %) | 39 / 142 (27 %) | 0.086 |

The packet matches the demo's size (7 kB), but it covers only half of the
fix's hunks in the files it kept, and about a quarter of all of them. As a
starting context this is thin. As a pointer into files the agent then
reads, it is cheap.

Rows: `scripts/bench/briefed-ab/results/briefer-arms-ba1/subq-2026-10-12/`
(`judgments.csv`, `summary.json` with SubQ divergences, `lines.json`).

## Chat router (routes-v5 held out) and web chat goldens

The router eval and the goldens target any `/v1/systemone` door through the
existing `hosted_http` decision profile, so no harness change was needed:

```sh
set -a; . ~/work/.secrets/subq.env; set +a
export CODER_DECISION_PROFILE=hosted_http CODER_DECISION_URL=https://api.subq.ai \
       CODER_DECISION_MODEL=subq-snap-preview CODER_DECISION_KEY="$SUBQ_API_KEY"
ROUTER_EVAL_SPLIT=held_out ROUTER_EVAL_MAPS=on \
  cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
OPENAGENTS_PRODUCT_KB_EMBEDDINGS="" cargo run -p coder --bin chat-goldens -- router
```

Router eval, all 274 held-out rows. Snap is read on Jev's map, as the
worker reads any model that is not Clef. The comparison rows are the
published numbers in
[2026-10-11-gemini-vertex-router.md](../coder/measurements/2026-10-11-gemini-vertex-router.md):

| Judge | Route acc. | Canned prec. / cov. | Dispatch prec. / recall | Gym prec. / recall | Refusal recall | Latency p50 / p95 |
|---|---|---|---|---|---|---|
| Jev (TypeSafe), `calibration-v2` | 91.5 % | 98.9 / 66.9 % | 98.0 % | 88.9 % recall | 91.3 % | ~0.9 s |
| Gemini 3.8 Flash, Jev's map | 93.4 % | 98.0 / 87.5 % | 100 % | 90.9 % recall | 100 % | 4.8 s |
| Clef-Flash flat, its map | 69.1 % | 94.6 / 33.3 % | 93.8 % | 43.2 % recall | 40.0 % | |
| **SubQ Snap, Jev's map** | **51.8 %** | **81.0 / 37.2 %** (34/42) | **- / 0 %** (0/47) | 27.3 / 16.2 % | 70.0 % | **0.42 / 0.95 s** |

- **Critical errors.** Snap served 8 wrong prepared answers: 4
  `capability.missing` rows got `presentation.elsewhere`, and 3 Gym rows got
  `eval.credit.mine`. That puts canned precision at 81 %, below the 98 %
  floor (`crates/gym/gates/router-v1.json`). It never offered Coder: 46
  rows were read as `cli` with none right (10 are labeled `cli`), most of
  them `work.dispatch` rows, so `work.dispatch` recall was 2 % (1 of 47) and
  dispatch recall 0 of 47. The eval and Gym routes (`eval.run`,
  `eval.check`, `eval.result`) got 0 of 22 right.
- What it gets right: `meta` 98 % recall, `general` and `smalltalk` 100 %,
  every pasted secret caught, refusal precision 100 %.
- The route question set was written and tuned against Jev. Snap reads
  the same `criteria` objects differently on the work/cli boundary.
  Changing the question wording to suit Snap is not something a door swap
  should require.

Goldens, router mode (126 cases):

| Judge | Route right | Answer right | Pass | Critical-flow failures |
|---|---|---|---|---|
| Jev, `calibration-v2` (#11195, 123 cases) | 120 (97.6 %) | 108 (87.8 %) | | |
| Gemini, Jev's map (first 76 cases) | 75 (98.7 %) | 71 (93.4 %) | 0 (time) | |
| Clef-Flash, its map (123 cases) | 110 (89.4 %) | 56 (45.5 %) | | |
| **SubQ Snap, Jev's map** | **103 (81.7 %)** | **88 (69.8 %)** | **78 (62 %)** | **14** |

Launch bar: not met (65 % right against the 90 % bar). The judge's time
per turn was 1.28 s at p50 and 1.75 s at p90, and 4 cases went over the
2 s budget. Every code-work phrasing went to `cli` (as in the eval), and
`starters.plugins`, `coder.sync`, `github.connect_repo` and
`account.connect_computer` failed.

## Latency at context sizes

Wall time from this Mac (upload included), `/v1/systemone` with one noul
and one choice, 5 runs each (2 at 300k):

| State | Input tokens | p50 | Min / max |
|---|---|---|---|
| 1k | 1,185 | 491 ms | 454 / 692 ms |
| 32k | 31,149 | 694 ms | 571 / 995 ms |
| 100k | 96,944 | **786 ms** | 752 / 1,526 ms |
| 300k | 295,165 | | 1,648 / 3,664 ms |
| subq-full (21 requests, about 190k average, 9 to 17 questions) | 192,330 mean | 3.0 s | p95 3.6 s |
| Per-file Noul, 80-line head (252 requests) | 1,739 mean | 418 ms | p95 485, max 844 ms |

**The claim of under 1 s for 100k tokens holds** (786 ms p50, including
the upload of about 380 kB). As the preview warns, the tail is variable:
one 100k request took 1.5 s, one image request 1.8 s and one 300k request
3.7 s.

Claims we could not check: the 384K context (it is not in the docs; 295k
was accepted), and "61.7 on Decision Index v0.2.1 against Jev's 57.9". On
our own router set Snap is far behind Jev (51.8 % against 91.5 %).

## Cost

| Run | Spend |
|---|---|
| Briefer: subq / subq-perfile-full / subq-full / line ranges | $0.022 / $0.219 / $0.202 / $0.086 |
| Router eval, held out (3.85M input tokens, plus a 7-row pilot) | about $0.20 |
| Goldens, router mode (not metered per call; about 14k tokens per turn) | about $0.10 |
| Latency sweep and smoke | about $0.06 |
| **Total** | **about $0.90**, on the account's free credits |

At $0.05 per million input tokens, a per-file briefing costs about $0.001 per
issue and a whole-candidate-set request about $0.01. A router turn
(about 14k tokens) costs about $0.0007, roughly a twentieth of Gemini on
Vertex.

## Wiring (opt-in)

- `coder solver --decisions subq` and `coder issue-run --decider subq`,
  also per point (for example `--decisions relevance=subq`), ask SubQ Snap
  for the issue kind and the per-file relevance Nouls. The door is
  `subq_door()` in `crates/coder-new/src/issue_run/decide.rs`: the Jev
  client at `jev_hosted::SUBQ` with `jev_hosted::SUBQ_MODEL`, the key from
  `SUBQ_API_KEY` or `~/work/.secrets/subq.env`, a 30 s timeout and 2
  retries. No other door asks it. `auto` does not include it.
- Tests: `the_subq_door_is_the_jev_client_at_subqs_url_and_model`, the
  parse tests in `issue_run/tests.rs` and `solver/tests.rs`, and the
  ignored live test `the_subq_door_answers`.
- The router, the goldens and anything else that reads the decision
  profile can already use Snap through `hosted_http` (see the commands
  above). Nothing in the product is pointed at it.
- `scripts/bench/briefed-ab/briefer_arms.py` gains the `subq`,
  `subq-perfile-full` and `subq-full` arms, and `lines` for the line-range
  step.

## Recommendation

- **Do not make Snap a default anywhere, and keep it off the chat router.**
  On our route set it reads 51.8 % with 81 % canned precision and never
  dispatches. That fails the router gate on every count it measures, even
  though it is the fastest door we have measured.
- **For the briefer's file judgments it is the best door, and the cheapest
  fast one.** On decide.rs's own question it gives top-1 0.76 against
  hosted Clef's 0.57, at about $0.001 per issue in about 6 s. If the solver
  needs a door when Haiku is not available (`--decisions`), Snap beats Clef.
  Haiku stays the default Briefer (0.888 against 0.825 accuracy).
- **Worth a downstream trial: the demo-shaped request** (`subq-full`
  plus line ranges). It is one request per issue, takes under 4 s, gives
  the highest door precision, and produces a 7 to 16 kB packet. The next
  measurement is agent pass rate with that packet against Haiku's
  briefing, as briefer-arms.md describes, and it is tracked in
  [subq-context-curation.md](subq-context-curation.md).
- **Before any production traffic, request zero data retention.** The
  org default keeps every state, which means our users' code and chats,
  indefinitely.
- Re-measure when Snap leaves preview, or when the router question set is
  next reworded.

## Reproduce

```sh
cd scripts/bench/briefed-ab
set -a; . ~/work/.secrets/subq.env; set +a
# frozen ba1 candidates: briefer_arms_restore.py (needs filefind), or rebuild
# the candidate lists and clef/hosted rows from results/briefer-arms-ba1/judgments.csv
python3 briefer_arms.py run --plan plans/briefer-arms.json --arms subq,subq-perfile-full,subq-full
python3 briefer_arms.py lines --plan plans/briefer-arms.json
python3 briefer_arms.py report --plan plans/briefer-arms.json --md
python3 -m unittest test_briefer_arms.py
```
