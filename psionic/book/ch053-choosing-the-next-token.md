# 53. Choosing the next token

> **Status: unimplemented — design draft.** Nothing here runs yet.

A generator needs a rule for turning logits into the next token. This chapter
defines that rule as a contract: a named random number generator, stated tie
rules, stop conditions and a grammar mask that makes a malformed output
impossible rather than detected afterward.

## What this chapter covers

- A named counter-based RNG with explicit seed and counter progression, so a
  seeded run reproduces on any host.
- Temperature, top-k and top-p, each with a stated tie rule.
- Stop conditions, streamed tokens and cancellation.
- Constrained decoding: the vocabulary is masked to a grammar or a JSON schema
  before sampling, using chapter 9's tokenizer tables. A token outside the
  grammar is never offered.
- Upstream sampling defaults, such as gpt-oss's temperature 1.0 and top-p 1.0,
  become explicit parameters, not hidden defaults.
- llama.cpp seeds `std::mt19937`, so you cannot match its seeded runs. You
  compare greedy outputs and distributions instead.

## What it builds

- A planned sampler library and a grammar compiler that produces per-step
  vocabulary masks. Chapter 64 implements the sampling primitives it needs
  without this chapter's full scope.

## Dependencies

- [Chapter 9](ch009-text-becomes-a-model-request.md): tokenizer tables for
  grammar masks.
- [Chapter 13](ch013-read-the-options.md): logits and probabilities.
- [Chapter 64](ch064-execute-gpt-oss-20b.md) and
  [chapter 77](ch077-call-a-tool.md): the first consumers.

## Done means

- The same seed and counter produce the same token sequence on two hosts.
- Sampled frequencies over many seeds match the stated distribution within a
  statistical bound, for each of temperature, top-k and top-p.
- Ties resolve by the stated rule in a fixture with equal logits.
- Under a grammar mask, no generated output fails to parse, across the grammar
  and schema fixtures.
- Cancellation stops generation within one step and publishes no partial
  token.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): grammar and
  JSON-schema fixtures.
- [gpt-oss target](../docs/targets/gpt-oss.md): upstream sampling defaults.
