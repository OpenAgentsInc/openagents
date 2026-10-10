# Model providers: every call site and its doors (2026-10-10)

Owner direction, 2026-10-10: Google first for everything, other providers as
fallbacks. We hold about 30k USD of prepaid Google credit on the GCP project
`openagentsgemini`; OpenRouter is out of credit and the Vercel AI Gateway
answers HTTP 402. So every model or embedding call made on **our** keys goes
to Gemini or a Google embedding model on Vertex AI first, with the older
providers behind it.

Two things never switch:

- **BYOK**: a call on the person's own keys (`model_access::Access::theirs`,
  desktop "Use my keys for everything", `openagents settings provider-key`)
  stays on the provider the person chose. Rows marked **BYOK**.
- **No Google credential**: a path that runs on a user's machine with no
  Google service account cannot reach Vertex, so its first door stays what the
  user configured. Rows marked **user machine**.

Vectors from different embedding models cannot be compared. An index built
with OpenAI's `text-embedding-3-small` keeps an OpenAI embedder for reading
until it is rebuilt with a Vertex model; pick the embedder from the index's
own model (`crates/coder/src/codebase.rs` `embedder_for`).

Google credentials: `inference::upstream::google::TokenSource` reads
`VERTEX_ACCESS_TOKEN`, `VERTEX_TOKEN_FILE`, `GOOGLE_APPLICATION_CREDENTIALS`,
or the GCE metadata server when `GCE_METADATA_HOST` / `K_SERVICE` is set.
Vertex embeddings: `knowledge::search::Embedder::vertex()`
(`text-embedding-005`, 768 dimensions; `KB_VERTEX_MODEL=gemini-embedding-001`
for Gemini's model). Vertex generation for Coder:
`coder::generate::vertex_door_from_env()` and `Door::behind`.

Owners of a row update it when they change the path. The gateway spec's
[section 2](gateway.md#2-what-exists-today) is the older summary.

## Generation

| Path | Call site | First door | Fallbacks |
| --- | --- | --- | --- |
| Chat worker (production, `coder-worker` on the worker VM) | `crates/coder/src/bin/coder-worker.rs` `main` door chain; `crates/coder/src/generate.rs` `vertex_door_from_env` | Vertex AI `gemini-3.8-flash` (global), `CODER_WORKER_VERTEX` / `VERTEX_PROJECT`, GCE metadata token | OpenRouter primary (`CODER_WORKER_PRIMARY`), the `CODER_DOOR_KEY` door, then `CODER_WORKER_BACKUPS` (`openrouter:gemini,openrouter:glm,vercel:gemini,vercel:glm`). With `CODER_INFERENCE_KEY` set, the inference gateway instead. |
| Inference gateway, `openagents/chat` (and `openagents/auto` when unjudged or judged `chat`) | `crates/inference/src/router.rs` `ClassTable::default`; adapters in `crates/gateway/src/inference_routes.rs` `upstreams` | Vertex AI `google/gemini-3.8-flash` (prepaid `google-credit`) | Pro door `openai/gpt-5.6-terra`, then OpenRouter `google/gemini-3.8-flash` |
| Inference gateway, `openagents/fast` | same | Vertex AI `google/gemini-3.8-flash` | Z.ai `zai/glm-5.3-flash`, then OpenRouter `google/gemini-3.8-flash` |
| Inference gateway, `openagents/classify` | same | Vertex AI `google/gemini-2.5-flash-lite` | Pro door `openai/gpt-5.6-luna`, then Z.ai `zai/glm-5.3-flash` |
| Inference gateway, `openagents/long` | same | Vertex AI `google/gemini-3.8-flash` | Z.ai `zai/glm-5.3-flash` |
| Inference gateway, `openagents/code`, `openagents/reason` | same | Pro door `openai/gpt-5.6-sol` (Vertex serves no Gemini Pro id; `gemini-3.8-pro` is 404) | `google/gemini-3.8-pro` on OpenRouter (reason); Z.ai (code) |
| Inference gateway, a model id such as `google/gemini-3.8-flash` (the Environments setup agent, `ENVIRONMENTS_MODEL`) | `crates/inference/src/router.rs` `plan`, step 4 (prepaid credit first) | Vertex AI | OpenRouter, then the Vercel AI Gateway (`privacy: standard` only) |
| Web chat vision | `crates/openagents-web/src/chat_vision.rs` | (owned by the chat-vision agent; row to be updated) | (to be updated) |
| Coder web search tool | `crates/coder-new` `web_search` | (owned by the web-search agent; row to be updated) | (to be updated) |

## Embeddings

| Path | Call site | First door | Fallbacks |
| --- | --- | --- | --- |
| Codebase index (chat worker) | `crates/coder/src/codebase.rs` `embedder`, `embedder_for` | Vertex `text-embedding-005` when the index was built with `CODER_CODEBASE_EMBEDDINGS=vertex` | An index built with OpenAI vectors reads with the Vercel AI Gateway, then OpenAI or OpenRouter |
| File finder | `scripts/filefind/filefind.py` `embed` | OpenRouter `openai/text-embedding-3-small` (256 dims) | None (runs without embeddings) |

More rows follow in this change's next commits.
