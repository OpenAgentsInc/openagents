# 83. A coding agent

> **Status: unimplemented — design draft.** Nothing here runs yet.

Devstral-Small-2-24B is Mistral's coding and tool-calling model (Apache-2.0,
FP8, 51.6 GB of files). This chapter runs it with Mistral's function-calling
format and measures it on a fixed set of coding tasks.

## What this chapter covers

- Mistral's function-calling format.
- Tool arguments constrained by chapter 53's grammar masks, so a malformed
  call is never generated.
- Long-context requests through tiled attention (chapter 41) and the prefix
  cache (chapter 176).
- A fixed coding-task set with checked outcomes.

## Dependencies

- [Chapter 81](ch081-execute-ministral-4.md): the Mistral tokenizer and chat
  format.
- [Chapter 82](ch082-original-fp8-weights.md): FP8 weights.
- [Chapter 53](ch053-choosing-the-next-token.md): constrained decoding.
- [Chapter 41](ch041-attention-in-tiles.md) and
  [chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): long context.

## Done means

- Token and logit references match for recorded prompts.
- Every generated tool call parses under its schema.
- The coding-task set reports pass rate, tokens and time per task on a named
  host.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
