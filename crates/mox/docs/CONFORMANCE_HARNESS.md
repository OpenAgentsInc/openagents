# Mox Conformance Harness

This document describes the reusable Ollama-to-Mox conformance harness added by
`MOX-117`.

The implementation lives in `crates/mox/mox-serve/src/conformance.rs` and stays
inside the reusable serve layer. It does not require app/UI participation.

## What It Covers

The harness normalizes and compares the subset of behavior the desktop depends
on:

- `tags` / installed-model discovery
- `show` / model detail facts
- `ps` / loaded-model state
- non-streaming `generate`
- streaming `generate`
- `embed`

Each check records one of four statuses:

- `passed`
- `failed`
- `unsupported`
- `intentional_difference`

`intentional_difference` is the escape hatch for cases where Mox is explicitly
not at Ollama parity yet but the gap is known and documented. That keeps the
cutover artifact honest instead of forcing silent drift or fake parity.

## Subject Types

Current reusable subject implementations:

- `OllamaHttpSubject`
  - live HTTP adapter over `/api/tags`, `/api/show`, `/api/ps`,
    `/api/generate`, and `/api/embed`
  - uses Ollama `_debug_render_only` on non-streaming generate cases so prompt
    rendering can be compared without sampling noise
- `RecordedConformanceSubject`
  - in-memory subject for tests or for callers that already have comparable Mox
    observations
  - suitable for current Mox integration while some parity surfaces are still
    landing

## Fixture-Driven Cases

`GenerateConformanceCase::from_generate_compatible_prompt_fixture(...)` builds a
non-streaming prompt-render case from the real golden prompt corpus in
`mox-models`.

Today that builder intentionally accepts only the subset of fixture cases that
can map honestly onto `/api/generate`:

- optional leading `system`
- one `user` turn
- `add_generation_prompt = true`

That is enough to anchor single-turn families such as `phi3` and `qwen2`
without pretending that multi-turn chat-template parity is already solved.

Embeddings cases also carry an explicit `EmbeddingParityBudget` from
`mox-runtime` so vector comparisons use the shared drift-budget policy instead
of one-off tolerance numbers.

## Report Shape

The harness emits a `ConformanceReport` JSON artifact with this top-level shape:

```json
{
  "suite_id": "qwen2-prompt-render",
  "baseline_subject": "ollama@http://127.0.0.1:11434",
  "candidate_subject": "mox-candidate",
  "checks": [
    {
      "surface": "generate",
      "case_id": "qwen2.default_system",
      "status": "intentional_difference",
      "detail": "current Mox prompt rendering is tracked separately in MOX-114; candidate marked unsupported: prompt rendering not yet implemented in Mox",
      "baseline": { "...": "..." },
      "candidate": { "...": "..." }
    }
  ],
  "summary": {
    "passed": 0,
    "failed": 0,
    "unsupported": 0,
    "intentional_differences": 1
  }
}
```

`ConformanceReport::cutover_ready()` returns `true` only when there are no
`failed` or `unsupported` checks.

## Documented Run

Repeatable CI-stable harness run for a supported model family:

```bash
cargo test -p mox-serve generate_case_builder_uses_real_qwen2_fixture \
  conformance_suite_records_intentional_candidate_gap \
  ollama_http_subject_normalizes_live_http_responses
```

Those tests cover:

- real `qwen2` prompt-fixture case construction from the golden corpus
- structured `intentional_difference` reporting for the current Mox prompt gap
- live HTTP normalization of Ollama `tags` / `show` / `ps` / `generate(stream)`
  / `embed` semantics via a local test server

## Controlled Local Validation

For a local cutover check against a real Ollama daemon:

1. Start Ollama with the target model family installed.
2. Build a `ConformanceSuite` using real golden fixture cases plus any
   candidate-specific expected differences.
3. Run the suite with:

```rust
let mut baseline = OllamaHttpSubject::new("http://127.0.0.1:11434")?;
let mut candidate = RecordedConformanceSubject::new("mox-candidate");
let report = run_conformance_suite(&suite, &mut baseline, &mut candidate)?;
write_conformance_report("target/mox-conformance.json", &report)?;
```

When Mox gains direct adapters for more surfaces, those adapters should
implement `ConformanceSubject` rather than inventing a second cutover harness.
