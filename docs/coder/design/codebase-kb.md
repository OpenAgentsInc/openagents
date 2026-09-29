# Codebase knowledge: the chat router's `codebase.kb` index

Status: implemented 2026-09-28 ([#9924](https://github.com/OpenAgentsInc/openagents/issues/9924)),
phase 5 of [the chat router](2026-09-28-chat-router.md#codebase-knowledge).

The `codebase.kb` route answers questions about how OpenAgents is built
("where is the chat worker's quota implemented", "what does kind 25900
carry") from an embedding index of this public repository at one pinned
commit, and cites `path:line` ranges at that commit. A question that needs
running code, live state, or tracing more source than the index holds is
escalated to a `work.dispatch` offer, so Coder does it on a computer.

## What is indexed

`knowledge::codebase` reads Git objects at the commit (`git ls-tree` and
`git cat-file --batch`), never a working tree, so uncommitted changes are
never indexed.

| Source | Chunking | Chunks at `e1d499def7` |
| --- | --- | --- |
| Markdown (`*.md`) | One per heading's section, split at blank lines past 1,600 characters; a `#` inside a fence is not a heading; sections with under 40 characters of text are skipped | 10,486 |
| Rust doc comments | Each `//!` block, and each `///` block with the item line it documents; blocks under 120 characters are skipped | 9,412 |

Left out: `docs/transcripts/`, `bench/`, `nips/official/` (upstream
copies), `knowledge/` (the coding knowledge base has its own index), any
`fixtures/`, `vendor/`, or `target/` directory. Only this repository is
indexed, never `alpha` or another private repository.

Each chunk keeps its path, first and last line, and a title (the heading
path, or the documented item's line).

## The file

- **Vectors.** OpenAI `text-embedding-3-small`, reduced to 256 components
  by keeping the leading ones and normalizing again (what the model's
  `dimensions` parameter does), and quantized to one signed byte per
  component with one scale per vector.
- **Format.** gzip of a magic line, a JSON header (schema
  `openagents.codebase-kb.v1`, repository, commit, model, chunks with their
  text, digests, scales), and the vectors.
- **Size.** 10.7 MB for 19,898 chunks at `e1d499def7`; the chunk text is
  most of it. Loaded whole in memory; a query is one linear scan (about
  5 million multiply-adds).
- **Cost.** A full build embeds about 3.8 million tokens: $0.08 at list
  price and six minutes through the AI Gateway.

## Where it lives and how it is refreshed

| | |
| --- | --- |
| Default path | `~/.cache/openagents/codebase-kb/codebase-kb.gz` |
| Override | `CODER_CODEBASE_KB=/path/to/codebase-kb.gz` |
| Build or refresh | `scripts/build-codebase-kb.sh [COMMIT] [OUT]` (default `origin/main`) |
| Embeddings key | `CODER_AI_GATEWAY_KEY` or `CODER_DOOR_KEY` (the chat worker's own door key), else `OPENAI_API_KEY`, else OpenRouter |

The file is an artifact, not a repository file: it is built on the
worker's host (or copied there) and is never committed. A refresh passes
the previous file as `--previous`, and a chunk whose embedded text is
unchanged reuses its vector, so moving the pin forward re-embeds only what
changed. The pin moves when an operator runs the script; the answer always
names the commit it read, so an older index is never mistaken for `main`.

## Answering

`coder::codebase` does four steps (all typed; nothing reads the question by
keyword):

1. **Retrieve.** Embed the question; the 10 nearest chunks by cosine
   similarity, at most 2 from one file.
2. **Judge.** One Jev request with independent Nouls: `needs_live` (does
   this need running code, live state, or tracing more source than docs and
   doc comments hold?) and `relevant_N` per candidate. Keep those at 0.5 or
   above, at most 6.
3. **Escalate or answer.** `needs_live` ≥ 0.6, or nothing relevant, is a
   dispatch. Otherwise the chat model answers from the kept excerpts only,
   citing their `path:start-end` ranges.
4. **Check.** Every `path:line` in the answer is parsed (a bounded field,
   after the route is chosen) and kept as a source only when it lies inside
   an excerpt the model was shown; a sources line naming the commit is
   appended.

For the router, `coder::codebase::Seam` implements
`router::seams::CodebaseKb`: `ground` returns the kept excerpts as passages
whose `id` and `source` are their `path:start-end`, the commit, and
`needs_dispatch` for step 3's escalation. Its recipient for the privacy
answer is the embedding provider.

## Evaluation

`codebase-kb eval` runs the 52 held-out questions in
`crates/coder/fixtures/chat-router/codebase-questions-v1.json` (42 that the
docs answer, each with gold paths and a reference answer; 10 that need a
computer) through the whole route and reports escalation recall, gold
retrieval, answer quality (a Jev Noul judging the answer against the
reference), citation validity (cited ranges inside the shown excerpts), and
citation precision against the gold paths. Results:
[the router eval measurement](../measurements/2026-09-28-chat-router-eval.md).

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a   # Jev
export CODER_AI_GATEWAY_KEY=...                   # embeddings and the answer model
cargo run --release -p coder --bin codebase-kb -- ask "what does kind 25900 carry?"
cargo run --release -p coder --bin codebase-kb -- eval --json /tmp/codebase-eval.json
```
