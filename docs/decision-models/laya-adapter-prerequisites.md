# Laya adapter prerequisites

**Status:** prerequisites for a Rust Laya adapter, recorded 2026-09-21.
Nothing here is implemented. The work is tracked under
[openagents#9486](https://github.com/OpenAgentsInc/openagents/issues/9486)
and the direction it lands is the Laya paragraph of
[decision-api.md](decision-api.md). Every claim about Laya comes from
[the reading](others/2026-09-19-laya.md) and the project pages it cites.
Where the project does not state something, this page says so rather than
filling the gap — claims the Laya project does not make must not be
invented for it.

A Rust adapter here means what it meant for Kev: this repository loads
the weights and serves `POST /v1/systemone` itself. Upstream is a Python
library with no HTTP endpoint — the community pull request that added one
was closed unmerged — so there is no service to be wire-compatible with,
and none of the package's API details bind the adapter. Community forks
that already implement the endpoint are tracked in
[awesome-jev](https://github.com/yibie/awesome-jev) and are reference
material, not sources to copy.

## The three checkpoints

Three checkpoints ship under Apache-2.0 with downloadable weights, all
bundled in one Hub repository:

| Checkpoint | Encoder | Parameters | Context | Role |
| --- | --- | --- | --- | --- |
| [`convaiinnovations/laya`](https://huggingface.co/convaiinnovations/laya) (repo root) | ModernBERT-large | 421M | 512 | English text, guardrails, email triage |
| [`convaiinnovations/laya-multilingual`](https://huggingface.co/convaiinnovations/laya-multilingual) (`multilingual` subfolder) | mmBERT-base | 322M | 1024, encoder up to 8k | 100+ languages |
| [`convaiinnovations/laya-typed-decisions`](https://huggingface.co/convaiinnovations/laya-typed-decisions) (`typed-decisions` subfolder) | ModernBERT-large | 421M | 1024 | the four typed-decisions workflows: invoice processing, customer service, security alerts, agent observability |

Two cautions on roles. `laya-typed-decisions` was fine-tuned on that
benchmark's own train split — its 0.766 is a fitted number, and the two
base checkpoints score below the majority-class baseline on the same
benchmark zero-shot. And "English" means Latin script: the English
checkpoint's 50,000-token BPE vocabulary shreds non-Latin scripts,
scoring 0.000 on Khmer at 0.952 confidence. Neither caution argues
against adapting; both argue that checkpoint identity and the routing
decision get pinned rather than implied.

## Where the artifacts live

- **Weights:** [Hugging Face `convaiinnovations/laya`](https://huggingface.co/convaiinnovations/laya).
  The English checkpoint sits at the repo root; the other two are
  subfolders fetched selectively through `allow_patterns` — about 808 MB
  for English and 647 MB for multilingual against roughly 2.5 GB for the
  bundle, as F32 and F16 safetensors. Standalone Hub repositories exist
  for `laya-multilingual` and `laya-typed-decisions`.
- **Code:** [GitHub `NandhaKishorM/laya`](https://github.com/NandhaKishorM/laya),
  distributed on PyPI as `laya`. It is a library whose dependencies
  carry no web framework; its inference entry point is
  `system_one(state, questions)` in `rl_agent_api.py`.
- **Provenance risk:** the upstream repository was created 2026-09-18
  and shipped thirteen PyPI releases in its first 48 hours, with no
  changelog and no versioned docs, and the model still calls itself
  `rl-agent` in its own responses and config. Names float here. Pin
  immutable Hub commit IDs and content digests, never release names or
  self-reported model names.

## What the adapter must pin

The requirement, from [decision-api.md](decision-api.md): licensed,
digested artifacts and explicit tokenizer and runtime configuration,
with English and multilingual checkpoints remaining identifiable.

- **Artifact digests.** SHA-256 and byte size for every file the runtime
  loads — each `model*.safetensors` shard, `config.json`, the decision
  head, and the tokenizer files — fetched from full immutable Hub commit
  IDs. The shape already exists: [`artifact-lock.json`](../kev/artifacts.md)
  beside each Kev fixture manifest.
- **Tokenizer configuration.** The two encoders do not share a
  tokenizer: English uses ModernBERT-large's 50,000-token BPE, and
  multilingual uses mmBERT-base's 256,000-token vocabulary. Pin the
  tokenizer per checkpoint, including the sequence budget the head
  consumes — `max_len` and `head_max_len` split each sequence between
  option prompts and state (512 with a 192-token head on English; 1024
  with a 256-token head on the other two; mmBERT's RoPE encoder reaches
  8,192).
- **Loaded-byte identity.** The server hashes the bytes it actually
  loads and publishes that content digest through `GET /v1/models`, and
  every recorded row retains it. [Model identity](../gym/model-identity.md)
  holds the schema and its limits. An upstream name is not an identity.
- **Execution configuration.** Backend, dtype, device, and the token
  bounds above record as execution identity beside the checkpoint
  digest, so a numerical change keeps the checkpoint identity while
  changing the measurement identity.

## Deployment prerequisites

What the project states:

- Weights are safetensors in F32 and F16; the three checkpoints total
  about 2.5 GB.
- Served checkpoints stay resident. Upstream's own guidance is
  `Router(preload=True)`: the lazy default keeps one model hot and
  rebuilds on every language switch — a 7.4 s median reload on CPU and
  10.3 s on T4 — while preloaded serving measures 32.8 ms on GPU and
  193–464 ms on CPU per request.
- The checkpoints ship over-confident. `laya` carries fitted
  temperatures bucketed by question type and option count;
  `laya-multilingual` ships with no fitted temperatures at all. Fit
  calibration on our own data before any probability is trusted.
- High-cardinality choice needs a raised `head_max_len` or a two-step
  coarse-to-fine split; at default budgets a 77-option question gets
  about three tokens per label, which the project measures at 0.425 on
  Banking77.
- The architecture is a bidirectional encoder plus a decision head:
  every option is scored at its own `[MASK]` marker and softmaxed over
  that question's options, and every question in a call shares one
  batched forward pass with its own copy of the state.

What this repository adds:

- Product code is Rust. The forward path above is reimplemented, not
  bound; Python stays out of the serving path.
- The door serves `POST /v1/systemone` and `GET /v1/models` like the
  existing doors and joins the gateway's single admission path.
- This repository measures English and only English today, and recorded
  rows say so in a `language` field. Serving the multilingual checkpoint
  widens that claim and inherits the router acceptance below.

## The language router is a versioned policy

Upstream ships a `Router` that picks a checkpoint per request before the
forward pass: Unicode script detection across 22 alphabets plus a Latin
stopword distribution check, sub-millisecond in pure Python — 0.09 ms on
English, 0.54 ms on Devanagari, 0.73 ms on a large nested JSON state.
Every answer carries `routing` metadata naming the `model`, the `repo`,
and a human-readable `reason`; a caller can override with `model=` or
inspect a routing decision without inference through `router.route()`.

The router exists because confidence cannot detect an unreadable script:
across the project's 51-language sweep the English checkpoint's mean
confidence never drops below 0.885 while accuracy falls to zero. A
routing error is therefore invisible in the answer it produces, which is
why the router cannot be an implementation detail.

The contract, from [decision-api.md](decision-api.md) and the receipt
rules beside it:

- Any automatic language routing is itself a **versioned policy**,
  versioned and digested the way review and fallback policies are — not
  code that drifts with an upstream package.
- Strict model selection still applies: a request that names a
  checkpoint gets that checkpoint or an explicit failure. Routing is a
  declared, opt-in policy, and a routed call names **which checkpoint
  actually served**. The receipt binds requested and actual artifact
  identity to the call, because a later `GET /v1/models` lookup cannot
  reconstruct which checkpoint answered an earlier request. English and
  multilingual checkpoints must remain identifiable end to end.
- Routing errors and mixed-language input are **declared workloads**,
  measured per the tracking issue's acceptance:
  [openagents#9486](https://github.com/OpenAgentsInc/openagents/issues/9486)
  requires conformance, quality, calibration, coverage, cold and warm
  latency, throughput, memory, and cost evidence for each new backend on
  the workloads it declares. Upstream's confidence claims and another
  model's benchmark do not stand in for local validation — including
  upstream's own routed numbers, which are measured but paired against
  nothing we run.

## What the project does not state

Recorded so nobody fills the gaps from the project page:

- No HTTP server, no wire contract, and no published API schema. The
  pull request for a drop-in `/v1/systemone` server was closed unmerged.
- No training data or training scripts; the issue asking for them is
  open with no maintainer response. Stated training compute was 1.96
  hours on one GPU.
- No changelog, no versioned documentation, and no release discipline a
  pin can reference — which is why the pin is digests and commit IDs.
- No minimum hardware, no Rust support, and no tokenizer specification
  beyond the vocabularies the model cards name.
- No stated behavior for mixed-script states beyond the routing metadata
  shown above; how the router weighs a state that is half Latin and half
  Devanagari is not described.

An adapter that needs a fact the project does not state gets it by
measuring, not by quoting.
