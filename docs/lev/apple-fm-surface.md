# The Apple FM surface, as this workspace already knows it

**Status:** a reading of work that already happened. Lev is not the first
time these repositories have driven Apple's on-device model — it is the
fourth or fifth, and two of those lanes ran against live hardware. This page
collects what was established, names where it lives, and separates what was
measured from what stays assumed. It exists so the Lev roadmap starts from
evidence instead of from a framework reference.

None of the Apple FM code described here is in this repository today. The
`openagents` work was deleted with the TypeScript product roots on
2026-08-28 (`d613b8ea22`, `fae80bde79`) and again in the Rust rebuild on
2026-09-18 (`dabc08102f`). It survives in history. The `psionic` work is
still live in that repository's working tree.

## Two lineages

**`psionic` built the contracts.** Over 2026-03-10 to 2026-03-22, a Rust
crate grew into a complete transport-neutral model of the framework's
surface: the bridge substrate (`9f04fff2`), sessions (`f2fca39c`), typed
availability (`fe98c120`), generation options (`4b8c588b`), streaming
(`735fc055`), structured generation (`38872600`), tool calling
(`b1d1316d`), transcripts (`f63ed6cf`), typed errors (`98835e35`), and
adapter lifecycle (`6dd9bf7b`, `cd7fdf39`, `e2508e27`). It lives at
`~/work/psionic/crates/psionic-apple-fm`, about 6,200 lines.

**`openagents` built the product.** Over 2026-06-29 to 2026-07-21, the same
model went from an optional on-device "decider" in a desktop client
(`e4968c23f5`) to a neutral local inference provider (`97a0e41543`,
AFS-02) to an on-device agent router (`0edc7eae69`) with readiness
projection, advisory-only authority, and signed-helper enforcement
(`957e646f70`).

The second lineage is the one Lev most resembles, and the first is the one
Lev reimplements from.

## The bridge

Neither lineage linked the framework into Rust. Both supervised a small
Swift executable and spoke HTTP to it on loopback, and both audits concluded
that boundary was the right one.

The Swift package targets macOS 26 and uses `SystemLanguageModel.default`
and `LanguageModelSession`. The HTTP surface `psionic` froze as constants:

| Path | Purpose |
| --- | --- |
| `GET /health` | Availability, the unavailable reason, the serving model |
| `GET /v1/models` | Model info, supported use cases, supported guardrails |
| `POST /v1/chat/completions` | An OpenAI-shaped completion, for compatibility |
| `POST /v1/sessions` | Create a session with instructions and tools |
| `POST /v1/sessions/{id}` plus `/stream`, `/structured`, `/transcript`, `/adapter` | Respond, stream snapshots, generate against a schema, export the transcript, bind an adapter |
| `GET /v1/adapters` | Adapter inventory and compatibility |
| `POST /control/shutdown` | Supervised stop |

The default model id is `apple-foundation-model`.

## What the typed contract records

`psionic-apple-fm/src/contract.rs` is the most precise statement of the
framework's surface anywhere in the workspace, because it had to round-trip
against a live bridge. The parts Lev depends on:

- **Availability** is an enum, not a boolean. The unavailable reasons are
  `apple_intelligence_not_enabled`, `device_not_eligible`, and
  `model_not_ready`, with an explicit unknown variant for future values.
- **Use case** is `general` or `content_tagging`. A classification workload
  can declare itself as one.
- **Guardrails** are `default` or `permissive_content_transformations`.
- **Sampling** is `greedy` or `random`. Greedy takes no top-k, no
  probability threshold, and no seed, and the contract validates that.
  Random takes a top-k *or* a probability threshold, never both, plus an
  optional **seed**. Temperature is separate and must be non-negative.
  Maximum response tokens must be positive.
- **Structured generation** goes through a JSON schema wrapper that
  sanitizes and validates the schema before it reaches the runtime, with a
  title hint for Apple's normalization.
- **Errors** are a closed typed set: `exceeded_context_window_size`,
  `assets_unavailable`, `guardrail_violation`, `unsupported_guide`,
  `unsupported_language_or_locale`, `decoding_failure`, `rate_limited`,
  `concurrent_requests`, `refusal`, `invalid_generation_schema`,
  `tool_call_failed`, `adapter_not_found`, `adapter_incompatible`,
  `invalid_request`, and `server_error`.

Nothing in that contract carries a log-probability, a logit, a token
alternative, or a hidden state. The only fields with "probability" in the
name are sampling controls the caller sends. That absence is the single most
important fact on this page, and it is an absence in a contract built by
people who were trying to expose everything the runtime offered.

## The router precedent

On 2026-07-20, `0edc7eae69` replaced a prompted-JSON router with one built
on guided generation. The commit message states the reasoning directly: the
route shape is guaranteed by a runtime `DynamicGenerationSchema` whose
candidate field is constrained to the connected-agent set, "so constrained
sampling can only emit an admitted candidate," and the bridge assembles the
route JSON rather than parsing it out of prose.

That is a Choice question in everything but name, and it was verified
against the live on-device bridge: coding requests routed to one agent,
planning to another, mechanical edits to a third, "every output a
well-formed admitted route."

The accompanying design plan (`7e5d067c88`) states the rule Lev inherits
without change: **the model proposes, the host acts and reports.** The model
emits a typed decision; deterministic code executes it; the model's word is
never the evidence that anything happened. That rule exists because an
earlier free-text version hallucinated actions, and the honesty preamble
written to suppress that hallucination is still quoted in the plan.

What the router did not have, and what Lev adds, is a probability and a
reason to trust it.

## The adapter lane

`psionic` froze three specs on 2026-03-14 (`75dd4099`) and implemented a
Rust-native reader, writer, inventory validator, and lineage extractor
(`f5c1c738`):

- `docs/APPLE_FMADAPTER_PACKAGE_SPEC.md` — a `.fmadapter` package is a
  directory containing `metadata.json` and `adapter_weights.bin`, optionally
  a complete `draft.mil` plus `draft_weights.bin` pair. The weights file is
  not raw tensor bytes: it is a Core ML blob-storage container with a
  64-byte file header, a record count, 64-byte-aligned tensor records each
  beginning with the magic `0xdeadbeef`, and fp16 little-endian payloads.
  A package with correct metadata and correct values is still rejected if
  the container layout is wrong. Required metadata keys are
  `adapterIdentifier`, `baseModelSignature` — a 40-character lowercase hex
  string — and `loraRank`.
- `docs/APPLE_ADAPTER_DATASET_SPEC.md` — training data is UTF-8 JSON Lines,
  one message array per line, roles `system`, `user`, and `assistant`. At
  most one system message, first if present, carrying any tools. A
  `response_format` field may appear only on user messages and only for
  guided-generation records, and it must be fully specified. The final
  message must be `assistant`.
- `docs/APPLE_ADAPTER_LINEAGE_SPEC.md` — the lineage values that ride in the
  package's `creatorDefined` map.

Both specs were frozen from Apple's adapter training toolkit v26.0.0, which
the specs cite by absolute path. **That toolkit is not on this machine
today.** Getting it back, or a current version, is a prerequisite for any
Lev adapter work rather than an implementation detail.

The base model signature the live lane used when the runtime surfaced
nothing more specific is `9799725ff8e851184037110b422d891ad3b92ec1`. Treat
it as the shape of the anchor, not as the current value.

## What our own audits already flagged

The `psionic` bridge audit of 2026-03-10 is unusually direct about the gaps,
and every one of them is still a risk for Lev:

- **Token accounting was fiction.** The Swift handler computed
  `promptTokens = prompt.count / 4`. The audit's conclusion — "not truthful
  token receipts" — matters because the System One contract has a `usage`
  object and the mesh plan wants to meter on it. A later `openagents` commit
  (`eda852854d`, AFM-5) is titled "honest usage truth" for the same reason.
  Lev reports what the runtime actually gives it or reports nothing.
- **One session was shared across requests**, which the audit called "not a
  safe request-isolation contract." Lev's isolation rule exists because this
  failure already happened here once.
- **The Swift HTTP server was naive**, reading each connection once, with no
  streaming and real truncation risk on larger bodies.
- **Bridge diagnostics were suppressed**, with the child process's output
  sent to null.

The later `openagents` work added the discipline that the helper's code
signature is authoritative before it is launched (`957e646f70`,
`0919324b30`), which is the shape of supply-chain control a Lev sidecar
needs from the first commit rather than the fortieth.

## What stays unmeasured

Nothing in either lineage measured the model *as a decision model*. There is
no accuracy number, no calibration curve, no order-sensitivity result, and no
isolation probe against Apple FM anywhere in this workspace. The router was
verified as "well-formed and plausible," which is a shape check, not a
quality measurement.

Specifically unknown, and all of it needed before Lev's design can be
finalized:

- Whether greedy decoding is deterministic run to run on the same machine.
- Whether a fixed seed reproduces a sample exactly, across sessions and
  across process restarts.
- How sampling spread relates to difficulty at all, which is the entire
  premise of the L2 estimator.
- The real context window, and how much of it a state plus one question
  costs.
- Latency as a function of state length, and whether per-question sessions
  pay the full state cost every time.
- How often guardrails fire on ordinary decision-shaped inputs.
- Whether an adapter trained for band selection improves calibration enough
  to justify the retraining treadmill the OS update schedule imposes.

[`roadmap.md`](roadmap.md) step 1 is exactly this list.
