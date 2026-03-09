# Psionic Conformance Harness

This document describes the reusable Ollama-to-Psionic conformance harness added by
`PSI-117`.

The implementation lives in `crates/psionic/psionic-serve/src/conformance.rs` and stays
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

`intentional_difference` is the escape hatch for cases where Psionic is explicitly
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
  - in-memory subject for tests or for callers that already have comparable Psionic
    observations
  - suitable for current Psionic integration while some parity surfaces are still
    landing

## Fixture-Driven Cases

`GenerateConformanceCase::from_generate_compatible_prompt_fixture(...)` builds a
non-streaming prompt-render case from the real golden prompt corpus in
`psionic-models`.

Today that builder intentionally accepts only the subset of fixture cases that
can map honestly onto `/api/generate`:

- optional leading `system`
- one `user` turn
- `add_generation_prompt = true`

That is enough to anchor single-turn families such as `phi3` and `qwen2`
without pretending that multi-turn chat-template parity is already solved.

Embeddings cases also carry an explicit `EmbeddingParityBudget` from
`psionic-runtime` so vector comparisons use the shared drift-budget policy instead
of one-off tolerance numbers.

## Report Shape

The harness emits a `ConformanceReport` JSON artifact with this top-level shape:

```json
{
  "suite_id": "qwen2-prompt-render",
  "baseline_subject": "ollama@http://127.0.0.1:11434",
  "candidate_subject": "psionic-candidate",
  "checks": [
    {
      "surface": "generate",
      "case_id": "qwen2.default_system",
      "status": "intentional_difference",
      "detail": "current Psionic prompt rendering is tracked separately in PSI-114; candidate marked unsupported: prompt rendering not yet implemented in Psionic",
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

`PSI-138` adds a separate performance gate on top of that semantic gate:

```rust
let thresholds = CutoverPerformanceThresholds::default();
let performance = report.performance_gate(&thresholds);
let ready = report.cutover_ready_with_performance(&thresholds);
```

The default thresholds are ratio-based against the Ollama baseline for the
same case:

- generation total duration: candidate must stay within `1.25x`
- generation load duration: candidate must stay within `1.25x`
- generation prompt throughput: candidate must stay above `0.80x`
- generation decode throughput: candidate must stay above `0.80x`
- embeddings total duration: candidate must stay within `1.25x`
- embeddings load duration: candidate must stay within `1.25x`

If either side omits the required timing evidence for a compared `generate` or
`embed` case, the performance gate reports `insufficient_evidence` instead of
guessing. That is intentional: cutover should fail closed when performance
evidence is missing.

## Documented Run

Repeatable CI-stable harness run for a supported model family:

```bash
cargo test -p psionic-serve generate_case_builder_uses_real_qwen2_fixture \
  conformance_suite_records_intentional_candidate_gap \
  ollama_http_subject_normalizes_live_http_responses
```

Those tests cover:

- real `qwen2` prompt-fixture case construction from the golden corpus
- structured `intentional_difference` reporting for the current Psionic prompt gap
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
let mut candidate = RecordedConformanceSubject::new("psionic-candidate");
let report = run_conformance_suite(&suite, &mut baseline, &mut candidate)?;
write_conformance_report("target/psionic-conformance.json", &report)?;
```

When Psionic gains direct adapters for more surfaces, those adapters should
implement `ConformanceSubject` rather than inventing a second cutover harness.
