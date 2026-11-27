# 22. Retrieval by meaning

> **Status: partly implemented.** Reader-side unit-normalized 64-dim semantic
> encoding ([`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax),
> `#14058`), `#authority` hybrid BM25 + dense cosine RRF fusion and relevance
> refusal gating ([`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax)),
> the quantized disk-cached hybrid workspace index ([`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax)),
> and the 12-case retrieval eval harness ([`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax))
> run on `cpu`, grounded by [`openagents/examples/knowledge-semantic-retrieval.coax`](../examples/knowledge-semantic-retrieval.coax)
> and [`openagents/examples/knowledge-semantic-retrieval.out`](../examples/knowledge-semantic-retrieval.out),
> and power `KnowledgeSearch` in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax).
> Remaining: hosted/Psionic neural embedding models and typed decision reranking
> in `openagents/knowledge/gateway-embed.coax`.

## Premise

A knowledge base is only useful if the right entry is found. This chapter adds
search by meaning, using embeddings, and measures it against questions whose
source is known.

## Draft

The agent can't put the whole knowledge base in front of a model on every
turn. It has to pick. The archive has tried many ways to pick: embeddings and
similarity search in [Episodes 13](../../docs/transcripts/013.md) to
[16](../../docs/transcripts/016.md), a hosted code index in [Episode
107](../../docs/transcripts/107.md), a knowledge graph in [Episode
122](../../docs/transcripts/122.md), and repository maps in [Episodes
156](../../docs/transcripts/156.md) to [160](../../docs/transcripts/160.md).

For the knowledge base we use embedding search. Each entry gets a vector. A
question gets a vector. The nearest entries are the candidates, and a decision
model can rank them. We do not match keywords to pick entries, for the same
reason we do not match keywords to pick routes: a keyword rule works on the
examples its author thought of and fails without notice on the rest.

Retrieval is measured before it is trusted. You write questions whose answer
is in a known entry and count how often that entry comes back in the top few.
The count goes in the chapter.

Each answer cites the entries it used. If nothing relevant comes back, the
agent says it doesn't know.

Repository search is a different problem and belongs to Coder. Coder's map of
a project is a member in its own right, and it is the capability chapter 43
re-measures.

## What we build

A plan for what this chapter ships once the draft is approved:

- An embedding index over knowledge entries, built through the gateway.
- A retrieval step: nearest entries, an optional typed ranking, and citations.
- A retrieval test set with known sources, and its measured hit rate.

It builds on what exists today:

- [`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax) (deterministic reader-side 64-dim unit-normalized dense semantic encoder, `#14058`)
- [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax) (`#authority` `kb_rec_hit_admitted` relevance refusal gate and `kb_rec_hybrid_fuse` RRF fusion)
- [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax) and [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax) (hybrid inverted BM25 + quantized dense cosine retrieval and 12-case eval benchmark)
- [`psionic/lib/book-search`](../../psionic/lib/book-search)
- [`coax/lib/gateway`](../../coax/lib/gateway)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coax/lib/nostr/kb`](../../coax/lib/nostr/kb)

### The chapter's check

- **Source:** [`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax),
  [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax),
  [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax),
  [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax),
  [`openagents/examples/knowledge-semantic-retrieval.coax`](../examples/knowledge-semantic-retrieval.coax),
  and [`openagents/examples/knowledge-semantic-retrieval.out`](../examples/knowledge-semantic-retrieval.out).
- **Trace:** encodes three fixture passages with reader-side 64-dim unit-normalized
  vectors (`unit_norm=true`), ranks `coax/book/ch11-writing-checks.md` first for
  the `#authority` query (`sem_milli=733,0,0`, `fused_hits=2` under BM25 + dense
  RRF), and ranks `docs/transcripts/289.md` first for the Episode 289 System One
  Front query (`sem_milli=0,584,72`).
- **Refusal:** a stopword-only query returns `kb_embed == false`
  (`stopword_rejected=true`), and a below-cutoff out-of-corpus query returns
  `0` hits from `kb_rec_hybrid_fuse` and emits `"refused":true` with
  `We don't have an entry for that.` and zero citations.
- **Independent check:** `coax/bin/coax run openagents/examples/knowledge-semantic-retrieval.coax`
  and `--release` match [`openagents/examples/knowledge-semantic-retrieval.out`](../examples/knowledge-semantic-retrieval.out),
  alongside `#test` suites in [`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax),
  [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax),
  and the 12-case `kb-tool.coax eval` benchmark.

Shipped paths: [`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax), [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax), [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax), [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax) (`#14058`).

Planned paths: `openagents/knowledge/gateway-embed.coax`.

## Sources

- **Episodes:**
  - [Episode 13, *RAG First Principles*](../../docs/transcripts/013.md): retrieval first principles
  - [Episode 14, *Embeddings 101*](../../docs/transcripts/014.md): embeddings
  - [Episode 15, *Similarity Search*](../../docs/transcripts/015.md)
  - [Episode 23, *Embedding our Codebase*](../../docs/transcripts/023.md): embedding a codebase
  - [Episode 107, *Codebase Indexing via Greptile*](../../docs/transcripts/107.md): a hosted code index
  - [Episode 122, *Codebase Indexing via GraphRAG*](../../docs/transcripts/122.md): GraphRAG
  - [Episode 160, *Relevant Files*](../../docs/transcripts/160.md)
- **Old code at `coder-v1.0.0`, crates:** `knowledge`, `code-index`, `code-graph`
- **NIPs:** [NIP-KB](../../nostr/nips/openagents/NIP-KB.md), [NIP-CTX](../../nostr/nips/openagents/NIP-CTX.md)
- **Also:**
  - `recipes/retrieval-rerank.recipe.json` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2

## Open questions for the owner

1. Which model makes the embeddings: a hosted one through the gateway, or one
   we serve with Psionic?

## Source listing

Run the Chapter 22 example in [`openagents/examples/knowledge-semantic-retrieval.coax`](../examples/knowledge-semantic-retrieval.coax) (committed transcript [`openagents/examples/knowledge-semantic-retrieval.out`](../examples/knowledge-semantic-retrieval.out)):

<!-- source: openagents/examples/knowledge-semantic-retrieval.coax knowledge-semantic-retrieval -->
```jai
ksr_run :: () -> bool {
  // 1. Encode three passages with reader-side 64-dim unit-normalized vectors.
  paths := [3] string.[
    "coax/book/ch11-writing-checks.md",
    "docs/transcripts/289.md",
    "food/pizza.md"
  ];
  heads := [3] string.[
    "Authority and Mutation Checks",
    "Episode 289 OpenAgents",
    "Sourdough Pizza Recipe"
  ];
  bodies := [3] string.[
    "Files marked #authority require zero surviving mutants under coax mutate.",
    "We decided to build System One Front and a shared knowledge base.",
    "Bake dough at 500 degrees with mozzarella and basil."
  ];

  v0: [KB_VEC_DIM] f32;
  v1: [KB_VEC_DIM] f32;
  v2: [KB_VEC_DIM] f32;
  e0 := kb_embed(heads[0], paths[0], bodies[0], v0[..]);
  e1 := kb_embed(heads[1], paths[1], bodies[1], v1[..]);
  e2 := kb_embed(heads[2], paths[2], bodies[2], v2[..]);
  unit_ok := e0 && e1 && e2 && abs(bs_dot(v0[..], v0[..]) - 1.0) < 1e-4;

  // 2. Semantic cosine ranking for two distinct natural-language queries.
  vq1: [KB_VEC_DIM] f32;
  q1_ok := kb_embed("", "", "how does Coax #authority mutation checking work?", vq1[..]);
  d1 := [3] f64.[bs_dot(vq1[..], v0[..]), bs_dot(vq1[..], v1[..]), bs_dot(vq1[..], v2[..])];
  den1: [3] BsHit;
  nd1 := bs_top(d1[..], 0.0, den1[..]);

  // Combine dense semantic hits with BM25 lexical hits via #authority RRF fusion.
  bm0 := bs_bm25(3, 1, 3, 12, 12.0);
  bm1 := bs_bm25(1, 2, 3, 12, 12.0);
  lex1 := [2] BsHit.[BsHit.{passage = 0, score = bm0}, BsHit.{passage = 1, score = bm1}];
  fbuf: [3] f64;
  fused: [3] BsHit;
  nf1 := kb_rec_hybrid_fuse(lex1[..], den1[0..<nd1], fbuf[..], fused[..]);
  print("q1_top=% sem_milli=%,%,% fused_top=% fused_hits=%\n",
    paths[den1[0].passage],
    kb_rec_to_milli(d1[0]), kb_rec_to_milli(d1[1]), kb_rec_to_milli(d1[2]),
    paths[fused[0].passage], nf1);

  vq2: [KB_VEC_DIM] f32;
  q2_ok := kb_embed("", "", "what did we decide in episode 289 about System One Front?", vq2[..]);
  d2 := [3] f64.[bs_dot(vq2[..], v0[..]), bs_dot(vq2[..], v1[..]), bs_dot(vq2[..], v2[..])];
  den2: [3] BsHit;
  nd2 := bs_top(d2[..], 0.0, den2[..]);
  print("q2_top=% sem_milli=%,%,% unit_norm=%\n",
    paths[den2[0].passage],
    kb_rec_to_milli(d2[0]), kb_rec_to_milli(d2[1]), kb_rec_to_milli(d2[2]), unit_ok);

  // 3. Fail-closed refusals: stopword-only query and below-cutoff out-of-corpus query.
  vstop: [KB_VEC_DIM] f32;
  stop_ok := kb_embed("the and", "what", "in of", vstop[..]);

  weak_lex := [1] BsHit.[BsHit.{passage = 2, score = 0.2}];
  weak_den := [1] BsHit.[BsHit.{passage = 2, score = 0.1}];
  nf_ref := kb_rec_hybrid_fuse(weak_lex[..], weak_den[..], fbuf[..], fused[..]);

  no_hits: [1] KbRecHit;
  jb: [1024] u8;
  o: [1] Jo;
  jo_start(o[..], jb[..]);
  kb_rec_write_reply_json(o[..], "neapolitan gelato recipe", 2, 3, 3, no_hits[0..<0]);
  nodes: [64] JvNode;
  doc: [1] Jv;
  mb: [64] u8;
  ref_parsed := !o[0].bad && jv_parse(doc[..], jo_text(o[..]), nodes[..]);
  ref_flag := ref_parsed && jv_bool(doc[0], jv_get(doc[0], 0, "refused"));
  mn := ifx ref_parsed then jv_str(doc[0], jv_get(doc[0], 0, "message"), mb[..]) else 0;
  ref_msg := cast(string) mb[0..<mn];
  print("stopword_rejected=% cutoff_hits=% refused=% message=%\n",
    !stop_ok, nf_ref, ref_flag, ref_msg);

  return unit_ok && q1_ok && q2_ok &&
         den1[0].passage == 0 && fused[0].passage == 0 && nf1 >= 2 &&
         den2[0].passage == 1 && !stop_ok && nf_ref == 0 &&
         ref_flag && ref_msg == KB_REFUSE_MSG;
}
```

Output (`coax/bin/coax run openagents/examples/knowledge-semantic-retrieval.coax`, identical in `--release`):

```text
q1_top=coax/book/ch11-writing-checks.md sem_milli=733,0,0 fused_top=coax/book/ch11-writing-checks.md fused_hits=2
q2_top=docs/transcripts/289.md sem_milli=0,584,72 unit_norm=true
stopword_rejected=true cutoff_hits=0 refused=true message=We don't have an entry for that.
ok=true
```
