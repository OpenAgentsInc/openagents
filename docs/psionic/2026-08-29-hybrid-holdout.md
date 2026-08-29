# Hybrid holdout — 2026-08-29

```text
CLAIM
actor/session: cursor 357-holdout
base: 5387926dba99412b6f9d248631b602f069501da4
worktree/branch: .oa-worktrees/issue-357-holdout
scope: 27B greedy token-identity holdout + thinking-on wrap
paths: crates/psionic-gguf/src/tokenizer.rs, crates/psionic-gguf/src/generate.rs, crates/openagents-cli/src/inference.rs, docs/psionic/
hot files: crates/openagents-cli/src/inference.rs
hot contracts: docs/psionic/CLI.md (slice 9–10 ids unchanged)
verification: cargo test -p psionic-gguf; cargo test -p openagents-cli --test inference_test; 27B greedy 32-token ID match vs Ollama
claimed_at: 2026-08-29T19:34:00Z
```

- CLI source: `0.2.0-rc.21` (this landing). Holdout binary was the
  worktree `target/release/openagents` built from this tree.
- Graph: `qwen35_hybrid` (`graph.hybrid` teach id)
- Host: development Apple Silicon Mac
- Weights: not in git
- Ollama: `0.33.2`, tag `qwen3.8:27b-mtp-q8_0`
- Prompt: `hello` (CLI `--prompt hello`)
- Sample: greedy (`temperature=0`, `top_k=1` on Ollama; CLI greedy)

## Fixture

`write_qwen35_hybrid_fixture` greedy IDs are deterministic.
`until_gen_done_on_fixture` still uses the four-tensor stub
(`embed_lmhead`).

## Tokenizer wrap

Ollama thinking-on render for this tag is 11 tokens, not 9. The
assistant turn opens with `<think>\n`. `--json` `prompt.done` now
includes `ids`.

Prompt IDs (both sides):

```text
248045 846 198 14556 248046 198 248045 74455 198 248068 198
```

`twenty_seven_b_hello_prompt_ids_match_ollama_when_blob_present` checks
that list when the operator blob exists. CI has no weights.

## 27B greedy vs Ollama

First 32 generated token IDs match Ollama token-for-token.

```text
760 1156 369 4777 5315 328 14556 1 471 264 6557 40719 13 353 1220
5707 93040 321 17185 11 9976 424 4145 321 11321 13 2233 1144 364
3977 34775 1532
```

First token is `760` (`The`). First mismatch: **none**.

`--json` teach ids for slices 9–10 stayed `prefill.start`,
`prefill.pos`, `prefill.done`, `gen.step`, `gen.logits`, `gen.sample`,
`gen.stream`, `gen.stop.length`, `gen.stats`, `gen.done`. Hybrid files
also emit `graph.hybrid` after `prefill.start` (already in rc.20).

`--local` was not flipped.

Wall clock for `--max-tokens 32` on this host was about two minutes
after mmap. That is a quality receipt, not a tok/s claim. Speed is
#358.

Re-run after install:

```text
openagents inference run --gguf <27B blob> --prompt hello --max-tokens 32 --json
openagents inference bench --gguf <27B blob> --prompt hello --max-tokens 1 \
  --compare-ollama qwen3.8:27b-mtp-q8_0
```
