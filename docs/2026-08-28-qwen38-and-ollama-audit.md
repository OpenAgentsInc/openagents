# Audit: Qwen 3.8 and Ollama across OpenAgents and Psionic

Date: 2026-08-28
Scope: every place Qwen (through 3.8) and Ollama appear in `openagents` at
`0538404c45` and in `psionic` at `1b539a48` (pulled from `origin/main`
immediately before this audit). Sources are named at each claim; line numbers
are pinned so a reader can re-verify without re-deriving.
Related: `docs/coder/2026-08-26-psionic-local-inference-integration-analysis.md`
(its gap #2, "Psionic has no Qwen 3.8 family", is now closed — see §4);
`docs/coder/2026-08-27-inference-and-rust-coder-api-audit.md`;
`docs/2026-08-25-openagents-cli-rust-migration-tradeoffs.md`.

---

## 1. The history: Ollama came in through the TypeScript CLI

The first local-model support predates the Rust port. Three commits tell the
whole arc:

| Commit | Date | What it did |
|---|---|---|
| `586edf191f` | 2026-08-24 | **Add Ollama local model support to openagents coder** (`packages/openagents-cli/src/coder-ollama.ts`, closes #21). `--model ollama:<name>`, the `ollama` npm client, default host `http://127.0.0.1:11434`, streaming `ReplyChunk`s. Chat only — no tools, no delegation. |
| `fc1b2d9406` | 2026-08-24 | **Let a local model delegate.** Tools were declared on the thread, so a local session had no tool runtime. `ReplySource` gained optional `useTools`; the Ollama source learned the same loop as the thread lane with `MAX_TOOL_STEPS`. Children still hold server grants, so a local parent delegates exactly as a thread session does. |
| `f296672580` | 2026-08-24 | **Answer from a local model by default.** With no `--model` at all, a machine already running Ollama answers from its most recently modified model (a 300 ms `/api/tags` probe before the first prompt). Hosted backends stayed one `--model` away; `--offline` asked for neither. Also added conversation export. |

Two days later `1fd49258ed` (2026-08-26) retired the TypeScript CLI. The
`packages/openagents-cli` tree is gone from `main`; the surviving Ollama
capability is entirely in the Rust port. The TS-era design survives in the
port at full strength — and in several places the port is stronger. That
inventory is §2.

Also from the TS era, worth not losing from memory:
`44ada8baa3` "Teach the tools token economy, per model family" and
`2a631cf631` "Budget a tool result by the model family reading it" ran the
same local lane; `8502aaa7d9` set qwen to think low by default. These are the
kind of model-family-sensitive tuning the Rust lane inherits through the
proxy lane but not through Ollama itself (see §3.4).

## 2. What the Rust coder has today (checked at `0538404c45`)

The Rust port did not lose the feature; it re-derives it in
`crates/openagents-cli/src/runtime.rs` with more surface than the TS version
ever had.

### 2.1 The lane

- `Lane::Local(String)` at `runtime.rs:383`. `Lane::from_str` (`:406-408`)
  accepts `local`, `ollama`, and `ollama:<model>`; an empty string means
  "whatever is installed". `--model ollama:<name>`, `--lane local`, and the
  dedicated `--local` flag (`cli.rs:686-690`, "Nothing in the conversation
  leaves the machine and nothing is metered") all reach the same lane.
- Shift+tab walks only the hosted lanes (`LANES` at `:312-338`: `flash`,
  `free`); the local lane is opt-in, and `Lane::label` renders it as
  `Coder Local (<model>)` (`:499-500`) so the frame always names what is
  actually answering.
- Server endpoint: `OLLAMA_HOST = "http://127.0.0.1:11434"` (`:258`),
  overridden by `OPENAGENTS_OLLAMA_HOST` (`:1110-1114`).

### 2.2 Resolution, streaming, tools

- **Model resolution** — `resolve_local_model` (`:3195`): exact name first,
  then family prefix (a reader who pulled `qwen3.8:27b-mtp-q8_0` says
  `qwen3.8`), then most-recently-modified when nothing was named; a miss is
  refused by name with the list of what is installed. List source:
  `installed_local_models` (`:3147`, `GET /api/tags`, 5 s timeout, sorted by
  `modified_at`).
- **Streaming** — Ollama speaks newline-delimited JSON, not SSE, so
  `run_local_turn` (`:3225`) splits frames itself; `absorb_ollama` (`:3689`)
  reads `thinking` as reasoning, `content` as deltas, whole-tool-call
  `tool_calls` with object arguments, and usage from `prompt_eval_count` /
  `eval_count` on the `done` line.
- **Tools** — the full harness loop runs on the local lane, same
  `MAX_TOOL_STEPS = 100` backstop as the thread lane (`:266`), parallel tool
  batches, budget notices (`BUDGET_NOTICES` at `:276`), and the same
  budget-report ending (`step_local_report`, `:3432`, a no-tools round so the
  turn ends in a report instead of a failure). Call ids are minted locally
  (`local_{index}_{name}`) because Ollama issues none; `ollama_message`
  (`:3919`) translates back, with `arguments` as object and tool results
  named by `tool_name`.
- **Images** — `ollama_message` strips the `;base64,` prefix and sends raw
  base64 under `images`, which is Ollama's wire shape.

### 2.3 What a local session deliberately does not do

- No thread, no grant, no metered spend (`last_grant = None`, `:3239-3241`).
  A test proves the whole turn completes with the OpenAgents host unreachable
  (`runtime_test.rs:893`, `the_local_lane_answers_with_the_proxy_unreachable`).
- No transcript events uploaded: `cloud_history` off by default, local
  session files are the source of truth, and `finish()` on a threadless local
  session reports nothing (`runtime_test.rs:2213`).
- The lane is refused as a delegable child lane (children need a server
  grant); the peer keeps its grant when a local parent delegates, carried
  over verbatim from `fc1b2d9406`.

### 2.4 Test coverage and the benchmark path

`crates/openagents-cli/tests/runtime_test.rs` carries a full local-lane
section against a stub Ollama server: proxy-unreachable turn, family-name
resolution, bare-lane most-recent pick, missing-model refusal, no-server
failure, tool round-trip, images, budget report, threadless finish. The
benchmark side runs the same CLI: `bench/adapters/openagents_coder.py:97-107`
maps `ollama/<model>` to `--model ollama:<name>`, and
`crates/openagents-cli/tests/gym_run_test.rs:366-369` pins that inference.

**Graded local-lane rows on record** (`bench-results/tb2-quick.jsonl`,
append-only per `bench-results/README.md`):

| Recorded | Model | Accepted | Success | Wall clock |
|---|---|---|---|---|
| 2026-08-25 22:57 | `qwen3.8:27b-mtp-q8_0` | 1 of 2 | 50% | 1395.8 s |
| 2026-08-25 23:27 | `qwen3.8:27b-mtp-q8_0` | 1 of 2 | 50% | 1395.8 s |
| 2026-08-25 23:33 | `qwen3:0.6b` | 0 of 2 | 0% | 143.9 s |

The third row is the deliberate degradation probe from the README: the gate
caught the model change as the confounder. For scale, the cross-section
proxy rows are `gpt-5.6-luna` 9/12 (75%) and `glm-5.3-flash` 7/12 (58.3%).
**The local lane has no `tb2-cross-section` row yet** — that is the missing
measurement that would put `qwen3.8` on the same 12-task board as the hosted
lanes.

### 2.5 The server side (`--dev` coder API)

`crates/openagents-coder-api/src/routes.rs:166-178` accepts `lane: "local"`
and opens a transcript-only thread without minting a grant — the same
contract as Phoenix's `lane: "local"`. The stub catalog lists no local
model; a client running on Ollama does not need one.

### 2.6 Conformance gaps against the TS original

The port covers everything the TS source did and more, with four deliberate
or accidental differences worth naming:

1. **No default-local probe.** TS probed `/api/tags` before the first prompt
   and answered from the most recent local model when no `--model` was given
   (`f296672580`). The Rust CLI defaults to `flash` and requires an explicit
   `--lane local` / `--local` / `ollama:`. Defensible either way — the probe
   fires on every session and a hosted default is predictable — but the
   "reader who installed Ollama meant it" behavior is gone. Restore or
   explicitly retire; do not let it stay an accident of the port.
2. **No sampler or context controls.** Nothing sends Ollama `options`
   (`num_ctx`, `temperature`, `top_k`, `top_p`, seed), `think`, or
   `keep_alive`. Defaults are Ollama's defaults; `num_ctx` especially can
   silently truncate a long session below what the model could carry.
   `think`/`thinking` is never forced either way, so a thinking-default model
   spends context on scratch the harness never asked for. Related: the
   reasoning `--reasoning` flag is recorded on threads only (`:1592-1595`);
   the local lane has no equivalent.
3. **Mid-stream cancellation.** `tool_cancellation` (`:1056`) reaches tools,
   and a cancelled tool batch aborts the turn (`:3441` in the loop), but the
   stream read itself (`bytes.next().await`) has no cancellation path — a
   30 GB Q8_0 model mid-decode owns the turn until it finishes. TS aborted
   the fetch on signal.
4. **`max_steps` naming.** The budget-exhausted outcome reports `max_steps`
   on both lanes; fine, but the local lane's equivalent constant is
   `MAX_TOOL_STEPS`, and any reader greping one name finds only half the
   behavior.

None of these block a release. All of them are cheap relative to what the
lane already does.

## 3. The machine and the models on it

The development machine runs Ollama with two models (verified live via
`/api/tags` during this audit):

| Model | Size | Quant | Context | Capabilities |
|---|---|---|---|---|
| `qwen3.8:27b-mtp-q8_0` | 30.0 GB | Q8_0 | 262,144 | completion, tools, thinking, vision |
| `qwen3:0.6b` | 0.5 GB | Q4_K_M | 40,960 | completion, tools, thinking |

Notes on the big artifact:

- `mtp-q8_0` is the multi-token-prediction-enabled Q8_0 community GGUF of
  Qwen3.8-27B. It is the model the bench rows in §2.4 were graded with, and
  the one `resolve_local_model`'s family-prefix docstring cites by name
  (`runtime.rs:3191-3192`).
- Its 262,144-token context and `vision` capability are the artifact's
  claims, not ones this codebase has exercised: the local lane sends no
  `options`, so the served context is Ollama's own default resolution, and
  no image path has been graded through Ollama here.

## 4. Psionic: Qwen 3.8 is implemented, not planned

`psionic` was pulled to `origin/main` (`1b539a48`, 2026-08-17) at the start
of this audit. The previous integration analysis's claim that "Psionic has no
Qwen 3.8 model family" was true on 2026-08-26 and is **false now** — the
qwen38 program landed and closed out in this window.

### 4.1 Program status

Authoritative source: `psionic/docs/qwen38/` (README, MODEL_FACTS,
IMPLEMENTATION_ROADMAP, PSIONIC_GAP_ANALYSIS, FIRST_GGUF_TARGET,
UNSLOTH_GGUF_ARTIFACT_INDEX). Milestones, per the roadmap's own table:

| Milestone | Status |
|---|---|
| R0-R5 research, artifact facts, prompt/tokenizer contract, checkpoint admission, BF16 evidence, GGUF qualification | `implemented` |
| R6 native CPU generation | `implemented` |
| R7 native CUDA generation | `implemented` |
| R8 OpenAI-compatible serving (CPU + CUDA) | `implemented_early` |
| R9 comparator + release gate | `implemented` |
| R9A CPU MTP speculative decoding | `implemented` (correctness only; no acceleration claim) |
| R10 native Metal | `partial` |
| R11 native vision | `partial` (separately admitted vision artifact; parity vs pinned Transformers: image cosine 0.99779, video cosine 0.99831) |
| R12 training/adapters | `implemented_early` (bounded reference math, recovery, artifact parity, CPU LM-head hot-swap) |
| R13 beat the Unsloth-equivalent speed test | `planned` |

Release gate: `fixtures/qwen38/reports/qwen38_release_gate_v1.json` —
`status: "passed"`, clean checkout, Psionic revision `99283e19`, all seven
gates green, 28/28 pinned llama.cpp recurrent comparisons, byte-identical
`/apply-template` renders for `low`/`medium`/`xhigh`.

Model identity (MODEL_FACTS.md): `Qwen/Qwen3.8-27B` @ `1d4bf0f2ff60…`,
Apache-2.0, `image-text-to-text`, 27B with vision encoder, 64 layers
(3× Gated DeltaNet linear attention + 1× gated full attention)×16, hidden
5,120, vocab 248,320, native context 262,144, MTP layer in config. Serving
id `qwen3.8-27b`; decoder architecture `qwen3_5_text` (family-shared with
3.5/3.6, deliberately not aliased). The Qwen3.8 tokenizer is **not
byte-identical** to Qwen3.6's; thinking is on by default; `xhigh` is the
default reasoning effort; tools use Qwen XML-style framing.

First execution artifact: `Qwen3.8-27B-UD-Q3_K_XL.gguf` (13.4 GB, SHA-256
`00cf92e6…`) from `unsloth/Qwen3.8-27B-GGUF` @ `fdd03b8bbd27…`, admitted at
4,096-token CUDA context; measured allocator peak 13.39 GB on the 16 GB RTX
4080. CPU generation is ~11.0 tok/s greedy on that box (CUDA rows in the
gate report are per-2-token bounded probes, not throughput claims).

### 4.2 Psionic versus Ollama: what has actually been measured

`psionic/docs/QWEN35_OLLAMA_COMPARISON.md` is the canonical native-vs-Ollama
matrix — for **Qwen3.5**, on an idle RTX 4080, with per-run divergence
evidence, forced-explicit Ollama sampler settings, GPU-drain hygiene between
rows, and row-strength classification (`strong` / `weak_length_matched_only`
/ `mismatched`). Headline: Psionic's native CUDA lane leads Ollama on all
four sizes (0.8B/2B/4B/9B) in every contract, with `qwen3.5:2b` greedy an
exact token-id match (`strong`).

**No Qwen3.8-vs-Ollama matrix exists yet.** The qwen38 program's R13
("beat the pinned Unsloth-equivalent speed test") is `planned`, and the
comparison doc's methodology has not been re-run against a `qwen3.8` GGUF.
Until it is, the honest statement is: Psionic beats Ollama on the 3.5 family
it has measured; Qwen3.8 parity-or-better is expected but unproven; Ollama
remains the serving path for Qwen3.8 everywhere outside Psionic's gated
CUDA envelope (4,096 tokens, one GPU, one artifact).

### 4.3 The two roads to local Qwen 3.8 in Coder

1. **Ollama (shipped, versionless, works today).** `openagents coder
   --model ollama:qwen3.8:27b-mtp-q8_0` — or just `--local` — on any machine
   with the model pulled. No context ceiling beyond Ollama's default
   resolution, no claim surface beyond Ollama's own, tool calling through
   the model's `tools` capability.
2. **Psionic native (implemented, gated, not wired into Coder).**
   `psionic-openai-server` serves `qwen3.8-27b` on CPU/CUDA with thinking
   controls, XML tool calls, streaming, and stored-response replay. The
   planned integration shape is already written down in
   `docs/coder/2026-08-26-psionic-local-inference-integration-analysis.md`:
   an `openagents inference` lifecycle namespace managing a separate
   `psionic-openai-server` process, with Coder talking to it as a
   provider-neutral local endpoint. That document's other premise — that
   Psionic's executor publishes only completed generations — is now
   outdated in the roadmap's streaming claims and needs re-verification
   before anyone builds against it.

---

## 5. The full inventory (everything this audit found, in one table)

| Location | What | Kind |
|---|---|---|
| `crates/openagents-cli/src/runtime.rs:383,406-408` | `Lane::Local`, `ollama:` parsing | live |
| `crates/openagents-cli/src/runtime.rs:3147-3223` | model list + family-prefix resolution | live |
| `crates/openagents-cli/src/runtime.rs:3225-3429` | local turn: NDJSON streaming, tools, budget | live |
| `crates/openagents-cli/src/runtime.rs:3432-3486` | budget-report round over local chat | live |
| `crates/openagents-cli/src/runtime.rs:3689-3748` | `absorb_ollama` frame reader | live |
| `crates/openagents-cli/src/runtime.rs:3919-3971` | `ollama_message` wire shape | live |
| `crates/openagents-cli/src/runtime.rs:258,1110-1114` | host constant + `OPENAGENTS_OLLAMA_HOST` | live |
| `crates/openagents-cli/src/cli.rs:672-690` | `--model ollama:<m>`, `--local` | live |
| `crates/openagents-cli/tests/runtime_test.rs:878-1120,2213` | local-lane test section | live |
| `crates/openagents-coder-api/src/routes.rs:166-178` | `lane:"local"` transcript-only thread | live |
| `bench/adapters/openagents_coder.py:97-107` | `ollama/<model>` adapter mapping | live |
| `bench-results/tb2-quick.jsonl` | 3 graded local-lane rows | evidence |
| `bench/suites/tb2-cross-section.md` | local-lane invocation documented, row missing | plan |
| `crates/openagents-cli/src/coder/tui.rs:1162` | " New in v0.1.1 " card | chrome |
| `packages/openagents-cli/src/coder-ollama.ts` (via `586edf191f`,`fc1b2d9406`,`f296672580`) | TS original, retired with the CLI | history |
| `psionic/docs/qwen38/*` | full qwen38 program docs | live (other repo) |
| `psionic/docs/QWEN35_OLLAMA_COMPARISON.md` | 3.5-vs-Ollama canonical matrix | evidence (other repo) |
| `psionic/fixtures/qwen38/reports/qwen38_release_gate_v1.json` | passed release gate | evidence (other repo) |

---

## 6. What is actually missing (ranked)

1. **A local-lane `tb2-cross-section` row for `qwen3.8:27b-mtp-q8_0`.** The
   suite's local-lane invocation is already documented in
   `bench/suites/tb2-cross-section.md`; the run just has not been recorded.
   This is the single highest-value missing measurement: it puts the local
   Qwen3.8 lane on the same board as luna (75%) and glm-flash (58.3%).
2. **Decision: default-local probe.** Restore the TS behavior (probe, then
   answer locally when a server exists) or write down that the Rust CLI
   defaults hosted on purpose. Leaving it undocumented is how the next
   port-vs-original audit rediscovers it.
3. **Ollama request controls.** At minimum `num_ctx` and a `think` switch;
   ideally the same reasoning-effort vocabulary the thread lane records.
   Without them the local lane cannot be tuned and cannot be compared
   honestly against a tuned hosted lane.
4. **Mid-stream cancellation** on the local lane's stream read.
5. **The Psionic wiring** (the `openagents inference` plan) — big, planned,
   and explicitly out of scope for a CLI release; it stays a separate track.
6. **A Qwen3.8-vs-Ollama matrix in Psionic** (R13) — closes the "expected
   but unproven" performance claim.

Items 1-4 are the honest, small scope of "add Ollama support to coder
v0.2.0": the support exists; what v0.2.0 should carry is the measurement,
the two behavior decisions, and the controls.
