# Psionic Prompt And Tokenizer Fixture Corpus

This document explains how the `PSI-118` golden corpus in `psionic-models` is
refreshed without drifting into giant, review-hostile blobs.

## Current Source Set

Tokenizer fixtures:

- `~/code/llama.cpp/models/ggml-vocab-llama-spm.gguf`
- `~/code/llama.cpp/models/ggml-vocab-qwen2.gguf`
- `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`

Prompt/template fixtures:

- `~/code/llama.cpp/models/ggml-vocab-phi-3.gguf`
- `~/code/llama.cpp/models/ggml-vocab-qwen2.gguf`
- `~/code/llama.cpp/models/ggml-vocab-command-r.gguf`
- `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`

Ollama reference files used for template-name and stop-default cross-checks:

- `~/code/ollama/template/index.json`
- `~/code/ollama/template/phi-3.json`
- `~/code/ollama/template/command-r.json`

## Refresh Rules

- Keep the corpus small and reviewable. Prefer digests, sampled token slices,
  and exact rendered prompts for a few representative cases over copying full
  vocabularies or giant templates into the repo.
- Preserve the separation between tokenizer facts and prompt-rendered outputs.
  Tokenizer fixture drift and prompt-render drift should fail in different
  places.
- When a real template is small enough to review, commit the raw template plus
  its digest.
- When a real template is large or dynamically rendered, commit a digest and a
  short excerpt instead of the full body.

## Why GPT-OSS Is Digest-Only

The real `gpt-oss` template is useful because it captures the `gpt-4o`
pretokenizer family and the current GPT-OSS chat-template shape. It is not a
good full golden-render fixture because:

- the template includes dynamic current-date rendering
- the rendered output changes with the available tool schema
- the raw template is large enough to be review-hostile

For that family, the fixture corpus therefore commits:

- tokenizer facts
- chat-template digest
- a short template excerpt

It intentionally does not commit full rendered prompt outputs.

## Practical Refresh Flow

1. Re-extract the GGUF tokenizer or chat-template facts from the real local
   source files.
2. Re-check template-name and stop-default mappings against the Ollama
   template files above.
3. Update the fixture constants in `crates/psionic/psionic-models/src/fixtures.rs`.
4. If a raw template changed, update its digest and any exact rendered prompt
   cases that rely on it.
5. Run:
   - `cargo test -p psionic-models -p psionic-serve`
   - `scripts/lint/ownership-boundary-check.sh`
6. Update `crates/psionic/docs/ROADMAP.md` after the issue lands, commit that
   roadmap refresh, and push it as a separate follow-up commit per the roadmap
   instructions.

## Review Checklist

- Does the fixture still point at a real GGUF or Ollama source file?
- Is the committed slice small enough to review in a normal diff?
- Are BOS/EOS defaults, stop defaults, named-template variants, and window
  pressure cases still covered?
- If a template moved to digest-only or back to raw form, is that justified in
  the diff and reflected in the notes?
