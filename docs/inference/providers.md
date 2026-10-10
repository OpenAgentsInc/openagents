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

Marks in the Path column: **BYOK** (the person's own keys, never switched),
**user machine** (runs with whatever the person has signed in to or
configured; no Google credential of ours is there, so Vertex cannot be
first), **tool** (operator dev, bench, or eval tool on our keys), **prod**
(a deployed service on our keys). Decision calls to Jev are TypeSafe's
product and keep TypeSafe's doors.

Owners of a row update it when they change the path. The gateway spec's
[section 2](gateway.md#2-what-exists-today) is the older summary.

## Generation

| Path | Call site | First door | Fallbacks |
| --- | --- | --- | --- |
| Chat worker turn (**prod**, `coder-worker-chat`) | `crates/coder/src/bin/coder-worker.rs` `main` door chain; `crates/coder/src/generate.rs` `vertex_door_from_env` | Vertex AI `gemini-3.8-flash` (global), `CODER_WORKER_VERTEX` / `VERTEX_PROJECT`, GCE metadata token | OpenRouter primary (`CODER_WORKER_PRIMARY`), the `CODER_DOOR_KEY` door, then `CODER_WORKER_BACKUPS` (`openrouter:gemini,openrouter:glm,vercel:gemini,vercel:glm`). With `CODER_INFERENCE_KEY` set, the inference gateway instead |
| Chat worker turn on the person's keys (**prod**, **BYOK**) | `coder-worker.rs` `their_door` | Their OpenRouter key | Their Vercel key; never ours |
| Dispatch sentence (personalization) (**prod** from the first worker release at or after `b91f3f1dbf`) | `crates/coder/src/router/personalize.rs` `Personalizer::named_behind` | Vertex AI `gemini-2.5-flash-lite`, thinking off (~0.5 s; budget 1.2 s), whenever the chat door's Vertex switch is on | The provider `CODER_PERSONALIZE` names (prod: OpenRouter `google/gemini-2.5-flash-lite`; or the gateway door's `glm`) |
| Dispatch sentence on the person's keys (**prod**, **BYOK**) | `personalize.rs` `Personalizer::theirs` | Their OpenRouter key | Their Vercel key; else the stem's own words |
| Router judge and "answers first" judge (**prod**) | `crates/coder/src/decision/profiles.rs`, `crates/jev-hosted/src/lib.rs` `resolve_with_fallbacks`; `crates/coder/src/router/judge.rs` `ask` | Jev on the Vercel AI Gateway (`typesafe-ai/jev`) | OpenRouter `typesafe/jev-1.13`, then TypeSafe `jev-1.13.0`. TypeSafe product: unchanged |
| Hosted decision worker (**prod**, `decision-worker.service`) | `crates/gateway/src/relay_worker.rs`, `src/bin/decision-worker.rs` | Jev on the Vercel AI Gateway | OpenRouter decisions, then TypeSafe (env template sets only TypeSafe). TypeSafe product: unchanged |
| Inference gateway (**prod**), `openagents/chat` (and `openagents/auto` when unjudged or judged `chat`) | `crates/inference/src/router.rs` `ClassTable::default`; adapters in `crates/gateway/src/inference_routes.rs` `upstreams` | Vertex AI `google/gemini-3.8-flash` (prepaid `google-credit`) | Pro door `openai/gpt-5.6-terra`, then OpenRouter `google/gemini-3.8-flash` |
| Inference gateway (**prod**), `openagents/fast` | same | Vertex AI `google/gemini-3.8-flash` | Z.ai `zai/glm-5.3-flash`, then OpenRouter `google/gemini-3.8-flash` |
| Inference gateway (**prod**), `openagents/classify` | same | Vertex AI `google/gemini-2.5-flash-lite` | Pro door `openai/gpt-5.6-luna`, then Z.ai `zai/glm-5.3-flash` |
| Inference gateway (**prod**), `openagents/long` | same | Vertex AI `google/gemini-3.8-flash` | Z.ai `zai/glm-5.3-flash` |
| Inference gateway (**prod**), `openagents/code`, `openagents/reason` | same | Pro door `openai/gpt-5.6-sol` (Vertex serves no Gemini Pro id; `gemini-3.8-pro` is 404) | `google/gemini-3.8-pro` on OpenRouter (reason); Z.ai (code) |
| Inference gateway (**prod**), a model id such as `google/gemini-3.8-flash` (the Environments setup agent, `ENVIRONMENTS_MODEL`) | `crates/inference/src/router.rs` `plan`, step 4 (prepaid credit first) | Vertex AI | OpenRouter, then the Vercel AI Gateway (`privacy: standard` only) |
| Inference gateway on the person's keys (**prod**, **BYOK**) | `crates/gateway/src/inference_byok.rs` `upstreams` | The workspace's saved key (OpenRouter, Vercel, Anthropic, OpenAI, or Google) | Only their keys |
| Inference gateway `openagents/auto` class (**prod**) | `inference_routes.rs` `JevClass` | Jev on TypeSafe direct | None. TypeSafe product: unchanged |
| Web chat vision (**prod**, openagents-web) | `crates/openagents-web/src/chat_vision.rs` `endpoint`, `call` | The inference gateway sidecar, `google/gemini-3.8-flash` (its chat chain, Vertex first) | The gateway's chain. **Owned by the chat-vision agent; row to be updated** |
| Environments setup agent (**prod**, web admin) | `crates/coder-environment-operator/src/studio.rs` | The inference gateway sidecar `/v1/responses` | The gateway's chain; else the Codex login |
| Web chat (**prod**) | `crates/openagents-web/src/ask.rs` `Worker::door` | Relay to the chat worker | The worker's chain |
| Hosted plugin-eval runner (**tool**, `openagents-eval-runner.service`) | `crates/eval-runner/src/config.rs` `from_env`; `crates/ext-eval/src/{proxy,live,run}.rs` | `CODER_DOOR_URL` (default Vercel AI Gateway, now HTTP 402), `google/gemini-3.8-flash` | None for the pinned run door (a run pins one door so results compare); Jev: Vercel, OpenRouter, TypeSafe. Not switched: see "Remaining" |
| Coder CLI turn (**user machine**) | `crates/coder/src/delegate_door.rs` `choose` | Microcoder in process | Claude Code CLI, Codex CLI, `Door::from_env` (`CODER_DOOR_KEY`), stub |
| Microcoder steps (**user machine**; cloud step on our keys) | `crates/coder/src/delegate_door/microcoder.rs` `lineup`, `provided`, `theirs` | Codex login `gpt-6.1-sol` | Claude Code `opus`, then OpenAgents cloud (relay to the chat worker, Vertex first there); **BYOK** under `mine`: their OpenRouter, then Vercel |
| Codebase KB answers (**tool**, builds the prod index) | `crates/coder/src/bin/codebase-kb.rs` `Composer::from_env` | Vertex AI `gemini-3.8-flash` when the Vertex switch is on (`VERTEX_PROJECT` + a Google credential) | The `CODER_DOOR_KEY` door, then OpenAI `gpt-4.1-mini`, then OpenRouter Gemini (first one with a key) |
| Product KB live eval (**tool**) | `crates/coder/src/product_kb/eval.rs` `Endpoint::from_env` | Gemini API (`generativelanguage.googleapis.com`, `GEMINI_API_KEY`) | OpenRouter Gemini |
| UI format bench (**tool**) | `crates/coder/src/bin/ui-format-bench.rs` `main` | `CODER_DOOR_URL` or the Vercel AI Gateway | None |
| Verse agents and villagers (**user machine**) | `crates/verse/src/brain.rs` `pick_door` | `ResponsesDoor::from_env` (`CODER_DOOR_KEY`) | The OpenAgents door, lane `free` (`OPENAGENTS_API_KEY`) |
| Knowledge harvest (**user machine**) | `crates/knowledge/src/harvest.rs` `CodexProposer`, `OpenRouterProposer` | Codex login `gpt-6-luna` | OpenRouter with `--provider openrouter` |
| Terminal-Bench Microcoder (**user machine**, bench) | `crates/microcoder/src/main.rs` `go`; `crates/microcoder-loop/src/{door,claude,vertex,models}.rs` | Codex login `gpt-6.1-sol` | By `--provider`: OpenRouter, Vertex (OpenAI-compatible endpoint, `~/.openagents/vertex-token`), Claude CLI, or the OpenAgents door |
| Microcoder autostarted repository runs (**user machine**) | `crates/microcoder/src/repository.rs` | The route's generation endpoint (Codex, Claude, sessions) | Capacity failover |
| Microluna backup transport (**user machine**, bench) | `crates/microluna/src/openrouter.rs`; `crates/coder-one/src/micro.rs` `codex_wire` | Codex login | OpenRouter Responses (`CODER_ONE_MICROLUNA_PROVIDER=openrouter`) |
| Coder One (**user machine**) | `crates/coder-one/src/generate.rs`, `main.rs` | The OpenAgents door `/v1/responses`, lane `free`/`flash`/`pro` (our upstream keys behind it) | None |
| coder-new chat, plugins, background agents (**user machine**, **BYOK**) | `crates/coder-new/src/provider.rs` `build`, `live.rs`, `programmatic.rs`, `fleet_app.rs` | The person's OpenRouter key, `openrouter/free` by default | Retries only |
| coder-new `web_search` (**user machine**, **BYOK**) | `crates/coder-new/src/web_tools.rs` `Searcher::choose`, `search` | Exa (`EXA_API_KEY`) | OpenRouter `web` plugin on `google/gemini-3.5-flash` with the chat's OpenRouter key. **Owned by the web-search agent; row to be updated** |
| coder-new Jev tool (**user machine**) | `crates/coder-new/src/jev_plugin.rs`, `plugin_tools.rs` `jev_client` | The saved key at TypeSafe or Vercel `/typesafe` | `jev_hosted::resolve`: local key, Vercel or OpenRouter env keys, the hosted decision worker. TypeSafe product |
| coder-new issue run decisions (**user machine**) | `crates/coder-new/src/issue_run/decide.rs` `jev_door`, `clef_door` | TypeSafe `jev-latest` (issue kind); local Ollama `clef-flash` (per-file questions) | Each other, by mode |
| `openagents inference` (**user machine**) | `crates/openagents-cli/src/inference.rs` | `api.openagents.com/v1` (the inference gateway) | The gateway's chain |
| Plugin test authoring (**user machine**) | `crates/openagents-cli/src/ext_eval_init.rs`, `eval_engine.rs` | `Door::from_env` (`CODER_DOOR_*`) | **BYOK** stored key (`google/gemini-3.8-flash`), then a local bridge over Claude Code, then Codex |
| Voyager curriculum (**user machine**, research) | `crates/voyager/src/curriculum.rs` `Responses::from_manifest` | The manifest's `/v1/responses` door | None |
| Gym run learning (**user machine**) | `crates/gym/src/runs_learning.rs` | `jev_hosted::resolve` (TypeSafe) | The hosted decision worker. TypeSafe product |
| Pylon provider jobs (**user machine**, provider) | `crates/gateway/src/inference_pylon.rs`; `crates/pylon/src/engine.rs` | The provider's loopback `psionic-openai-server` | None |
| Anthropic key check (**prod**, **BYOK**) | `crates/openagents-web/src/cloud/byo.rs` `check` | `api.anthropic.com` `GET /v1/models` (validates, no generation) | None |
| Mobile and desktop key tests (**BYOK**) | `crates/openagents-mobile/src/provider_keys.rs`; `crates/openagents-desktop/src/settings.rs`; `crates/model-access` | Key checks only (OpenRouter `/key`, Vercel `/credits`, TypeSafe); chat goes over the relay to the chat worker | None |
| File-finding bench plan and judge stages (**tool**) | `scripts/bench/file-finding-bench.py` `cmd_plan`, `cmd_rerank`, judge | OpenRouter `anthropic/claude-sonnet-5.5` or the Claude CLI (plan); TypeSafe Jev (judge) | None |
| Simulated users QA (**tool**) | `scripts/qa/simulated_users.py` `ask_model` | OpenRouter `stealth/space-bunny-alpha` (retired model) | The same model again, then `claude:haiku` via the Claude CLI |
| Relevance and score benches (**tool**) | `scripts/bench/clef-relevance-bench.py`; `training/score-probe/probe.py`; `crates/gym/suites/eval_independence_v1.py`; `bench/jev-lifecycle/gateway.py` | Local Clef/Kev/Lev or TypeSafe `jev-latest`; Vercel `typesafe-ai/jev` (lifecycle) | None |
| Delegation study broker (**tool**) | `bench/delegation-study/broker.py` | `api.anthropic.com` `/v1/messages` (metered forwarder) | None |
| Local gateway and worker (**tool**) | `scripts/dev/inference-local.sh` | The local inference gateway (Vertex with `VERTEX_ACCESS_TOKEN`) | Its chain |

## Embeddings

| Path | Call site | First door | Fallbacks |
| --- | --- | --- | --- |
| Codebase index (**prod**, chat worker) | `crates/coder/src/codebase.rs` `embedder`, `embedder_for` | Vertex `text-embedding-005` for an index built with `CODER_CODEBASE_EMBEDDINGS=vertex` (questions read with the index's own model) | An index built with OpenAI vectors reads with the Vercel AI Gateway, then `Embedder::from_env` (OpenAI, then OpenRouter) |
| Product KB (**prod**, chat worker) | `crates/coder/src/product_kb.rs` `embedder_from_env` | Prod (release `156b301714`): `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=vertex`, `KB_VERTEX_MODEL=gemini-embedding-001` on the VM's metadata token. Unset: `Embedder::house` (Vertex on a Google credential) | Vertex has no same-model fallback (vectors cannot mix); unset with no Google credential: OpenAI, then OpenRouter; `gateway`: Vercel, OpenRouter, OpenAI |
| Gym KB seam (**prod**, chat worker) | `crates/coder/src/gym_kb.rs` | As the product KB (`product_kb::embedder_from_env`) | As the product KB |
| Seams on the person's keys (**prod**, **BYOK**) | `crates/coder/src/router/seams.rs` `TheirKeys::embedder`; `knowledge::search::Embedder::theirs` | Their OpenRouter key (`text-embedding-3-small`) | Their Vercel key; never ours |
| Knowledge search, `kb` CLI and `microcoder --kb-embeddings` (**user machine** / **tool**) | `crates/knowledge/src/search.rs` `Embedder::chosen`, `Embedder::house`; `crates/knowledge/src/cli.rs` | **BYOK** first; then Vertex `text-embedding-005` when a Google credential is here (`Embedder::google`, project `openagentsgemini` unless named) | OpenAI (`OPENAI_API_KEY`), then OpenRouter; `KB_GOOGLE_FIRST=off` skips Vertex |
| Agent recall (**user machine**) | `crates/coder/src/task/agent_recall.rs` `live` | `Embedder::house` (Vertex on a Google credential) | OpenAI, then OpenRouter; else BM25 |
| Delegate recipe knowledge (**user machine**) | `crates/coder-delegate/src/recipe.rs` | `Embedder::house` | OpenAI, then OpenRouter; else lexical |
| File finder (**tool**; also run by the coder-new issue run) | `scripts/filefind/filefind.py` `embed`, `embed_key`, `index_embedder` | Vertex `text-embedding-005`, 256 dims, when a Google credential is here (`GOOGLE_APPLICATION_CREDENTIALS`, `~/work/.secrets/gcp-mvp-automation.json`, GCE metadata, or `FILEFIND_EMBEDDINGS=vertex` with gcloud) | OpenRouter `openai/text-embedding-3-small`; a query reads with the model its index holds; none: runs without embeddings |
| Knowledge harvest near-duplicates (**user machine**) | `crates/knowledge/src/cli.rs` `harvest_and_report` | `Embedder::chosen` (as knowledge search) | As knowledge search |

## Remaining

- Personalization goes live with the next chat worker release at or after
  `b91f3f1dbf`; the worker already has `VERTEX_PROJECT` and the metadata
  token, so no env change is needed. Until then prod personalizes on
  OpenRouter (HTTP 402), so the dispatch sentence ends with the stem's own
  words.
- Hosted plugin-eval runner: its run door is pinned per run so results
  compare; moving it to Vertex is a change to the eval protocol, not a door
  swap.
- User-machine paths stay on the person's sign-ins and keys: there is no
  Google credential of ours on their computer.
