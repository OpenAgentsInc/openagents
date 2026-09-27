# Why Microcoder did worse on the Codex login than on OpenRouter

September 26, 2026. Tracking:
[#9683](https://github.com/OpenAgentsInc/openagents/issues/9683). This
follows up the [154-entry check](2026-09-26-kb-154-check.md), whose result
put the drop on the route: `--provider codex` against `--provider
openrouter`, same model (GPT-6 Luna), same effort (`medium`).

**Result: the Codex route never let the model reason.** On the Codex route,
a step asked for its action as a call to a strict function tool,
`next_action`. On the OpenRouter route, it asked for the same JSON schema as
a structured output format. Given the function tool, GPT-6 Luna answered
with **0 reasoning tokens** in all 12 such calls on the Codex endpoint
(the dump run's 2 steps and 10 probes), at `low`, `medium`, and `high`
effort. It did the same in 2 of 3 calls on
OpenRouter's own Responses API. With the schema as the output format, the
same request reasoned on both endpoints. The records agree: Codex-route
steps wrote a mean of 447 output tokens against 1,254 on OpenRouter, on
every task where both ran. Effort was sent and applied, and neither route
truncated the prompt or dropped context. The fix makes the Codex step send
the schema as its output format, as OpenRouter does, and no tool.
After the fix, Codex steps reason, but less than OpenRouter's at the same
`medium` effort. Four `medium` runs on `gsea-proteomics` and
`fin-saccr-rwa` averaged 334–660 output tokens a step and all failed. Two
runs at `--effort xhigh` averaged about 1,600 and 4,000, near or above
OpenRouter's `medium`, and both passed. Four and two runs are too few to
set a rate, but they point the same way as the probes.

## Rules followed

- **Tasks.** Only excluded development tasks: `sound-change-cascade` for
  the payload dumps and probes, and `gsea-proteomics` and `fin-saccr-rwa`
  for the verification runs. The record comparison read runs on the 14
  excluded TB4 tasks and the 15 excluded TB2.1 sweep tasks only.
  `~/study-oos` and runs on held-out or Fable-fails tasks weren't read.
- **Host.** coderos-4080, in a separate worktree (`~/route-diff-wt`) with
  its own `CARGO_TARGET_DIR`, deleted afterwards. No `microcoder-study-*`
  binary or process was touched. At most 2 runs at a time, next to the two
  running studies.
- **Spend.** OpenRouter answers HTTP 402 at Microcoder's default request
  size (it reserves 65,536 output tokens), but it still accepts requests
  with `max_tokens` at 16,000. 16 OpenRouter probe calls cost about $0.01
  in all. Codex probe and verification calls are on the operator's login.

## Step 1: the two requests, side by side

`MICROCODER_DUMP_REQUESTS=<dir>` (new; see [The fix](#the-fix)) writes each
step's request body, as sent, with the usage its reply reported. Bodies
carry no credentials: those go in headers. Two 2-step runs on
`sound-change-cascade`, one per provider, gave these bodies. OpenRouter's
calls then failed with 402, so its replies came from the probes below.

| | Codex route (before the fix) | OpenRouter route |
| --- | --- | --- |
| Endpoint | `chatgpt.com/backend-api/codex/responses` (Responses API, streamed) | `openrouter.ai/api/v1/chat/completions` (Chat Completions, not streamed) |
| Model sent | `gpt-6-luna` | `openai/gpt-6-luna` (OpenRouter routes it to OpenAI) |
| Model reported | `gpt-6-luna` in all 764 recorded steps | `openai/gpt-6-luna` |
| Effort | `reasoning.effort: "medium"`. The response echoes `"effort": "medium"`, and `low`, `high`, and `xhigh` are echoed too | `reasoning.effort: "medium"` |
| Reasoning summary | `summary: "auto"`, echoed as `"detailed"` | not requested |
| Encrypted reasoning | `include: ["reasoning.encrypted_content"]`. Never sent back: every step is one stateless request | not requested |
| **Output shape** | **`tools: [next_action]`, a strict function tool, with `tool_choice: "auto"` and `parallel_tool_calls: false`** | **`response_format: json_schema` `next_action`, strict, the same schema**, plus `provider.require_parameters` and the `response-healing` plugin |
| System text | `instructions` = the system text + " Reply by calling next_action exactly once." | a `system` message with the system text |
| Step prompt | one user message, 22,557 characters at step 1 | one user message, 22,557 characters at step 1 |
| Input tokens, step 1 | 5,420 | 5,346 (the difference is the tool declaration and the added sentence) |
| `max_output_tokens` / `max_tokens` | not sent | not sent (OpenRouter reserves 65,536) |
| Temperature | not sent | not sent |
| Prompt caching | `prompt_cache_key` = the run's session | none sent; OpenAI caches automatically |
| Earlier turns | none: each step is one request whose prompt holds the history, the files, Jev's judgments, and the knowledge section | the same |

Both routes build the prompt with the same code (`run::prompt`), and the
dumps show the same prompt length on both, so neither route truncates the
prompt or drops the knowledge section or earlier steps.

**Malformed or empty replies.** On the Codex route, a reply with no
`next_action` call was a bad reply ("called no next_action tool"). On
OpenRouter, a reply that isn't the schema's JSON is one ("doesn't match the
requested shape"), after the response-healing plugin. Three bad replies in a
row end a run. Transient failures are retried inside one step: 3 times
(2, 4, 8 s) by Microluna on Codex, 2 times (0.5, 1 s) by the OpenRouter
client. The records don't count attempts.

## The probes

`~/route-diff-runs/route_probe.py` (Codex) and
`~/route-diff-runs/route_orprobe.py` (OpenRouter) on coderos-4080 replay a dumped body with one change and print the usage. Both
read their credentials from the files Microcoder uses and print nothing
else. The bodies are the Codex route's step 1 (5,420 input tokens) and
step 2 (8,757) of the `sound-change-cascade` dump run.

| Endpoint | Output shape | Effort | Reasoning tokens per call |
| --- | --- | --- | --- |
| Codex | function tool (as sent) | medium | **0, 0, 0** (step 1); **0, 0, 0** (step 2) |
| Codex | function tool, `tool_choice: "required"` | medium | 0 |
| Codex | function tool | high | **0, 0** |
| Codex | function tool | low | 0 |
| Codex | JSON schema output format | low | 0, 0 |
| Codex | JSON schema output format | medium | 39, 18, 63 (step 1); 38, 39, 25, 39, 36, 42, 71, 23, 58, 52, 41 (step 2, with and without the summary, encrypted content, or the system text as a developer message) |
| Codex | JSON schema output format | high | 57, 30 |
| Codex | JSON schema output format | xhigh | 250, 158 |
| OpenRouter Responses | function tool | medium | 0, 0 (step 1); 54 (step 2) |
| OpenRouter Responses | JSON schema output format | low | 46, 45 |
| OpenRouter Responses | JSON schema output format | medium | 171, 136 (step 1); 109, 116, 64 (step 2) |
| OpenRouter Responses | JSON schema output format | high | 134, 145 |
| OpenRouter Chat Completions (the OpenRouter route as sent) | JSON schema response format | medium | 479, 169, 88 (step 1) |

Three things follow:

1. **A function-tool step doesn't reason.** That holds on both endpoints,
   so it's the request's shape, not the Codex login.
2. **Effort reaches the Codex request and is applied** (low 0, medium about
   40, xhigh about 200 with the output format), but only when the model
   reasons at all.
3. **With the same output format, the Codex endpoint still reasons less
   than OpenRouter at the same effort** (about 40 against about 120 on the
   step-2 body at `medium`). Codex `medium` sits near OpenRouter `low`. Our
   request can't change that. It's how each service runs the model.

## Step 2: the records

`~/route-diff-runs/route_an.py` on coderos-4080 read every run record under
`~/.openagents/microcoder/runs`, `~/sweep-runs`, `~/kbcheck-runs`,
`~/kbcheck`, and `~/loop2` on the excluded tasks, with GPT-6 Luna at
`medium`. Records made before `summary.json` named its provider are
assigned by the model slug: `openai/gpt-6-luna` is OpenRouter, and the bare
`gpt-6-luna` is Codex.

| Route | Runs | Passed | Ended by bad replies | Median steps (other runs) | Mean output tokens per step | Bad replies / steps |
| --- | --- | --- | --- | --- | --- | --- |
| Codex (`provider: codex`) | 22 | **0** | 0 | 27 | **447** | 2 / 764 |
| Codex (before the provider field) | 14 | 1 | 11 (HTTP 429, the login's usage limit) | 16 | 423 | 33 / 149 |
| OpenRouter (`provider: openrouter`) | 33 | **6** | 8 (HTTP 402, out of credit) | 39 | **1,254** | 33 / 1,063 |
| OpenRouter (before the provider field) | 59 | 16 | 3 | 25.5 | 1,398 | 26 / 2,658 |

On every task both routes ran, the Codex runs wrote a fraction of the
output tokens per step (mean per run):

| Task | Codex | OpenRouter | Passed (Codex / OpenRouter) |
| --- | --- | --- | --- |
| `batched-eval-parity` | 414–498 | 629–1,268 | 0/4 · 0/5 |
| `fin-saccr-rwa` | 438–979 | 1,336–2,649 | 0/7 · 3/6 |
| `gsea-proteomics` | 282–472 | 691–1,052 | 0/3 · 3/6 |
| `hof-topology-interpenetration` | 277–311 | 1,218–1,414 | 0/4 · 0/5 |
| `sound-change-cascade` | 316–368 | 1,536–1,856 | 0/4 · 0/7 |

The records don't store reasoning tokens (a step keeps only its output
total), but the difference matches the probes: a Codex step's output was
the action alone, and an OpenRouter step's was reasoning plus the action.

The bad replies aren't the cause. The Codex route's two were a server
error and a flagged prompt. OpenRouter's were 26 HTTP 402s and 7 replies
that began with prose instead of the schema's JSON.

## The fix

In `crates/microcoder/src/models.rs`, a Codex step now sends what an
OpenRouter step sends:

- **No tool.** The `next_action` schema goes in the Responses API
  `text.format` as a strict `json_schema`, the counterpart of OpenRouter's
  `response_format`. `tools`, `tool_choice`, and `parallel_tool_calls` are
  left out.
- **The same system text.** The added "Reply by calling next_action exactly
  once." is gone.
- **The same parsing.** The reply's text is read the way
  `openrouter::Client::structured` reads it: its first complete JSON value,
  with or without a Markdown fence.

To support that, `microluna::Request` has a `text_format` field (`None`
everywhere else, so other callers send what they sent before),
`microluna::codex::body` writes it, and `microluna::oneshot::answer` returns
a reply's text with the same retries and pricing as `oneshot::call`.
`openrouter::ChatRequest::structured` builds the request
`Client::structured` sends, so a test can compare it.

- **Tests.** `codex_and_openrouter_steps_send_the_same_parts` builds both
  bodies for one step and checks that they carry the same system text,
  prompt, effort, and schema, with no tool, output cap, or temperature on
  either. Other new tests cover the text format in the Codex body,
  `oneshot::answer`, and parsing a fenced reply.
- **`MICROCODER_DUMP_REQUESTS=<dir>`** writes each step's body and reply
  usage (reasoning tokens included) as `<provider>-<pid>-<n>.json`, on
  both routes.
- `cargo test -p microcoder -p microluna -p openrouter -p knowledge` and
  `cargo clippy -p microluna -p microcoder -p openrouter -p knowledge -p
  coder-one --tests -D warnings` pass.

The door provider (`--provider door`) and the repository-native path still
use the function tool. They weren't part of this comparison.

## Verification

The fixed build (this commit's code, a debug build in the separate worktree)
ran on the Codex login with the kbcheck settings: `--provider codex --kb
candidates --max-steps 60 --max-usd 1.00 --max-minutes 30`, the host's
normal knowledge folder, and `MICROCODER_DUMP_REQUESTS` on. Two ran at a
time. Records, logs, and dumps are in `~/route-diff-runs/` on coderos-4080,
moved out of `~/.openagents/microcoder/runs` so the study's scans don't pick
them up.

| Run | Effort | Result | Steps | Time | Model cost (list price) | Reasoning tokens per step, mean (median) | Output tokens per step, mean | Steps with 0 reasoning | Bad replies |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `gsea-proteomics` 1 | medium | fail | 28 | 6:30 | $0.034 | 57 (40) | 334 | 1 | 0 |
| `gsea-proteomics` 2 | medium | fail | 17 | 5:13 | $0.020 | 49 (35) | 380 | 1 | 0 |
| `fin-saccr-rwa` 1 | medium | fail | 24 | 5:32 | $0.040 | 92 (51) | 519 | 1 | 0 |
| `fin-saccr-rwa` 2 | medium | fail | 23 | 5:54 | $0.042 | 143 (77) | 660 | 1 | 0 |
| `gsea-proteomics` | xhigh | **pass** | 16 | 9:54 | $0.042 | 915 | 1,581 | — | 0 |
| `fin-saccr-rwa` | xhigh | **pass** | 6 | 42:57 | unknown, $0.018–$0.50 | 2,419 | 3,976 | — | 0 (1 retried broken stream) |

- **The fix works as intended.** Every step sent the schema as its output
  format, every reply parsed, and steps reason: 4 of 92 `medium` steps had
  0 reasoning tokens, against every step before.
- **`medium` on Codex still isn't OpenRouter's `medium`.** These runs wrote
  about 330–660 output tokens a step, against 1,254 for OpenRouter-route
  runs, and 0 of 4 passed. Before the fix, Codex-route runs on these two
  tasks passed 0 of 10, and OpenRouter-route runs passed 6 of 12.
- **`xhigh` on Codex reached OpenRouter's output per step and passed both
  tasks**, but slowly on `fin-saccr-rwa`: 43 minutes for 6 steps, past the
  30-minute cap, which is checked only between steps. Two of its calls broke
  mid-stream, so its cost is a bound, not a figure.
- Spend: $0.18 of known list price in model calls for the verification
  runs, plus the `fin-saccr-rwa` xhigh run's unpriced calls (its whole run is
  at most $0.50), and about $0.01 on OpenRouter for the probes.

## What this means for earlier results

- Every Codex-route run before this fix, including Round 3 of the
  out-of-sample study and the 154-entry check, ran GPT-6 Luna with no
  reasoning. Comparisons between the routes, and between Codex-route runs
  and OpenRouter-route runs of other rounds, compare two different
  configurations.
- Round 3 runs on `microcoder-study-r3`, which was built before this fix.
  Its results stand as recorded, under that configuration. Switching the
  study's binary mid-round would be a loop change, so it stays as it is.
- Even after the fix, Codex `medium` reasons less than OpenRouter `medium`.
  Matching OpenRouter's reasoning on the Codex login may need a higher
  effort. That's a configuration question for the development set, not
  something this fix decides.
