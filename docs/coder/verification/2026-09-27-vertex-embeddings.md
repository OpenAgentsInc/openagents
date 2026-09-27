# Vertex AI embeddings verification — September 27, 2026

This record covers an opt-in Vertex AI embeddings provider for
knowledge-base search, for
[issue #9680](https://github.com/OpenAgentsInc/openagents/issues/9680). The
OpenAI key has no credit and OpenRouter is out of credit, so knowledge search
on the study host ranks by words alone. This provider lets a declared round,
or an operator, rank by embeddings again without either account.

## Delivered behavior

- **Model.** Google's `text-embedding-005` through Vertex AI's `predict` on
  the publisher model, in
  [`crates/knowledge/src/search/vertex.rs`](../../../crates/knowledge/src/search/vertex.rs).
  It was chosen over `gemini-embedding-001` because it takes up to 250
  inputs a request, where `gemini-embedding-001` takes one, and because its
  response reports `metadata.billableCharacterCount`, the unit it's priced
  in. Entries are embedded as `RETRIEVAL_DOCUMENT` and the query as
  `RETRIEVAL_QUERY`, with `autoTruncate` on.
- **Opt-in only.** `kb search` and the `kb harvest` commands take
  `--embeddings vertex`; a Microcoder run takes `--kb-embeddings vertex`.
  Without the flag, `Embedder::from_env` is unchanged: OpenAI with a key,
  else OpenRouter. No study binary, round configuration, or default passes
  the flag. `--kb-lexical`, `--lexical`, and private input still win over
  it.
- **Configuration.** `KB_VERTEX_PROJECT` (or `GOOGLE_CLOUD_PROJECT`) names
  the project, and `KB_VERTEX_LOCATION` the region, default `us-central1`.
  `KB_VERTEX_URL` replaces the whole models URL, for a test endpoint.
- **Authentication.** With `VERTEX_TOKEN_FILE` set, the token is read from
  that file before every request, as the `--provider vertex` model provider
  does. Otherwise it comes from `gcloud auth print-access-token`, which
  honors `CLOUDSDK_CONFIG`, and is reused for up to 30 minutes. The token is
  sent only as the bearer and is never printed or written.
- **Batching.** A request holds at most 250 inputs and an estimated 20,000
  tokens. The estimate errs high: a token per two characters, capped at the
  2,048 tokens Vertex keeps of one input, plus one.
- **Cost.** A request's cost is its billable characters at the list price,
  $0.000025 per 1,000 characters for online requests (Google Cloud's Vertex
  AI pricing page, "Embeddings for Text (Excluding Gemini Embedding)",
  retrieved 2026-09-27). The basis is `list_price`. A response without the
  count leaves the cost unknown (`null`), never $0. A request refused with
  an error status, or a request never sent for lack of a token, costs
  nothing. A failure after an earlier request of the same call was billed
  leaves the cost unknown.
- **No mixing.** Vectors are cached under `vertex/text-embedding-005`,
  apart from `openai/text-embedding-3-small`. The per-process query cache is
  keyed by model too. `search::rank` refuses to rank when the query's model
  isn't the index's, or when a cached vector's length differs from the
  query's.
- **Records.** `summary.json` `retrieval` names
  `"embedding_provider": "vertex"`,
  `"embedding_model": "vertex/text-embedding-005"`, and
  `"embedding_cost_basis": "list_price"`, beside `mode`.
- **Shared client.** `openrouter::Client::post_json` exposes the client's
  existing bearer, retry, and error mapping for other JSON APIs; errors from
  the Vertex path name Vertex AI.

## Checks

All from this Mac, with a dedicated target directory and `debug = 0`.

- `cargo test -p knowledge --lib`: 100 passed, 1 ignored (the live smoke).
  The 12 new tests in `search::vertex::tests` run against a local fake
  `predict` endpoint:
  - `a_request_has_the_predict_shape_and_a_list_price_cost`: the path
    `…/publishers/google/models/text-embedding-005:predict`, the bearer from
    the token file, the instances with task types, `autoTruncate`, and the
    cost of 26 billable characters.
  - `inputs_are_batched_by_count_and_by_tokens` and
    `a_long_list_goes_out_in_several_requests_in_order`: 600 short inputs go
    out as 250, 250, and 100; 20 long ones as 9, 9, and 2; 260 inputs over
    the fake come back in order from two requests, with the summed cost.
  - `an_error_status_is_a_refusal_that_names_vertex`,
    `no_token_is_a_refusal_before_anything_is_sent`,
    `a_failure_after_a_billed_request_leaves_the_cost_unknown`, and
    `a_malformed_response_is_not_a_refusal`: error mapping and whether each
    failure may have been billed.
  - `a_response_without_billable_characters_has_an_unknown_cost`: a search's
    cost is `None`, not $0.
  - `a_search_ranks_with_vertex_and_costs_its_characters`: a second search
    embeds only its query, as `RETRIEVAL_QUERY`.
  - `the_cache_keeps_each_models_vectors_apart`: after another model filled
    a shared cache file, Vertex embeds all three entries itself, and the file
    holds both models' vectors under separate keys.
  - `an_index_refuses_a_query_from_another_model`: `rank` refuses a query
    from another model and a vector of another length.
- `cargo test -p microcoder --lib`: 106 passed, 5 ignored. The retrieval record
  test checks that it names the Vertex provider, model, and cost basis.
- `cargo clippy -p knowledge -p microcoder -p openrouter --all-targets -- -D warnings`:
  clean.
- `cargo fmt` on the three crates: clean.

## Live smoke

One call, from this Mac, on 2026-09-27:

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
KB_VERTEX_PROJECT=openagentsgemini \
cargo test -p knowledge --lib live_a_handful -- --ignored --nocapture
```

It embedded three short test entries and the query "kernel two-sample test"
in one request through `Embedder::vertex()` and a scratch cache file:

```
live: model vertex/text-embedding-005, 3 vectors of 768 dimensions, top hit stats.mmd (1.000), cost Some(4.350000000000001e-6) (about 0.0000043500 expected for 174 non-whitespace characters)
```

Vertex AI reported 174 billable characters, so the measured cost is
**$0.00000435** at list price. No Microcoder task, model run, or benchmark
ran, and the smoke didn't touch `~/.openagents/knowledge/embeddings.json`.

## Limits

- A declared study round must name this option in its configuration before
  any of its runs use it. It changes what ranks the knowledge base, so
  switching a round to it mid-round breaks the study's rules.
- The vectors aren't comparable with `text-embedding-3-small`'s. The first
  search with Vertex embeds every entry once. Entries' search text is
  short, so for the current base of 188 entries that is a fraction of a
  cent.
- The token estimate for batching is a heuristic. An input far denser than
  two characters a token could push a request past 20,000 tokens, and
  Vertex would refuse it with HTTP 400; the search then ranks by words and
  says why.
- The `gcloud` token path runs a subprocess from async code. It blocks for
  about a second, once per 30 minutes per process.
- Only `text-embedding-005` is supported; there's no flag for another
  Vertex model.
