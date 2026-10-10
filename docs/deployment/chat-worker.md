# The chat worker: OpenAgents chat in the OpenAgents app

A new OpenAgents app user can chat with OpenAgents before connecting a
computer; work for a computer is dispatched to Coder there. That chat,
*OpenAgents chat*, is served by one `coder-worker` running open,
with no usage limit, answering on Space Bunny Alpha through OpenRouter first
and the gateway door's Gemini Flash lane after. This page is the serving
decision, its admission rules, and the runbook. Every job is recorded in the
usage log; [chat-worker-usage.md](chat-worker-usage.md) shows how to read it.

## The serving path

```text
phone (device key) --NIP-CJ 25900, NIP-44--> relay.openagents.com --> chat worker
phone <--27000 partials, 26900 result------- relay.openagents.com <-- chat worker --> Vertex AI (Gemini), then OpenRouter, then AI gateway
```

- **Wire.** Each turn is a NIP-CJ conversation job
  ([`nips/openagents/NIP-CJ.md`](../../nips/openagents/NIP-CJ.md)): a kind
  `25900` request signed by the phone's device key and encrypted with NIP-44
  to the worker, and the worker's `27000` partial feedback and `26900` result
  back. The relay sees ciphertext and routing tags only and keeps nothing:
  every kind is ephemeral.
- **Endpoint.** The worker's public key is compiled into the app
  (`basic_coder::WORKER` in `crates/openagents-mobile`):
  `32c078952ff8b1f1d6f431e30fb240b0d1f91e30f977844557e8267390e3599b`. Its
  relay is `wss://relay.openagents.com`.
- **Authentication.** The identity the user already has: the device key that
  the app creates on first launch signs the request and authenticates to the
  relay (NIP-42). There is no account, sign-in, or key to paste.
- **Model, and failover across providers (2026-10-09).** Every turn goes
  down a chain of doors (`crates/coder/src/generate.rs`, `FallbackDoor`)
  and is answered by the first that starts answering, so the person never
  sees a provider's failure while any door can answer. On 2026-10-09 the
  old two-door chain failed every chat: the primary, Space Bunny Alpha,
  answered 404 "No endpoints found" (OpenRouter retired it) and the one
  fallback, the Vercel AI Gateway, answered 402 `insufficient_funds`. The
  chain now, in order:
  1. The OpenAgents inference gateway (`CODER_INFERENCE_KEY`,
     `docs/inference/gateway.md`), when its service key is set. Production's
     gateway is not deployed yet (staging's runs on loopback inside the
     staging service), so the production worker has no gateway door today;
     when production's gateway is deployed and its env file gets a service
     key, the gateway goes first and every door below stays behind it.
  2. Gemini on Vertex AI (since 2026-10-10, #11219): the same
     `google/gemini-3.8-flash` (Vertex id `gemini-3.8-flash`, global
     endpoint) on Google's own API in project `openagentsgemini`, billed to
     the prepaid Google credit, through Vertex's native
     `streamGenerateContent?alt=sse` at thinking level `low`
     (`CODER_WORKER_VERTEX`; on whenever `VERTEX_PROJECT` is set, `off`
     leaves it out). The token is the VM's service account's
     (`157437760789-compute@developer.gserviceaccount.com`, scope
     `cloud-platform`), read from the metadata server because the env file
     sets `GCE_METADATA_HOST=metadata.google.internal`, cached until a
     minute before it expires and fetched again after a 401. No key file is
     on the VM. Its journal lines name the host: `job … answered in … ms,
     … chars, by google/gemini-3.8-flash at aiplatform.googleapis.com`; a
     turn OpenRouter took says `at openrouter.ai`. When Vertex misses, the
     failover line names it (`door google/gemini-3.8-flash missed its first
     words … ; google/gemini-3.8-flash at https://openrouter.ai/api takes
     the turn`) and the result's `switched` names provider `other` ("The
     first model provider had a problem, so google/gemini-3.8-flash
     answered instead."), a word every app build knows.
  3. The primary, `google/gemini-3.8-flash` on OpenRouter at reasoning
     effort `low` (`CODER_WORKER_PRIMARY`; unset, Gemini whenever
     `OPENROUTER_API_KEY` is set; Space Bunny Alpha is retired).
  4. The door `CODER_DOOR_URL` and `CODER_WORKER_MODEL` name (left out when
     it is the primary again; production's is OpenRouter's Gemini since the
     hot fix, so it is).
  5. The backups (`CODER_WORKER_BACKUPS`; unset, every one whose key is
     here): `z-ai/glm-5.3-flash` on OpenRouter, then `google/gemini-3.8-flash`
     and `zai/glm-5.3-flash` on the Vercel AI Gateway (`AI_GATEWAY_API_KEY`),
     another account.

  A door that fails before its first words (any HTTP error, including 402,
  401/403, 404, 429, and 5xx, a connection that fails, a failure event, an
  empty stream, nothing within four seconds, or, while it streams its
  reasoning, no answer text within eight) hands the same turn to the next;
  the last door keeps its own retries. A door that answered 404 (model
  gone) sits out 30 minutes, one that answered 401, 402, or 403 (key or
  account) five, so later turns do not pay its failed request first; when
  every door is benched, every door is asked. The journal logs each switch
  (`door google/gemini-3.8-flash missed its first words after … ms (door:
  the model endpoint answered HTTP 402: …); z-ai/glm-5.3-flash at
  https://openrouter.ai/api takes the turn`), each bench (`door … benched
  for 300 s (door)`), and every answer (`job … answered in … ms, … chars,
  by <model>`), and each result's `model` names the model that wrote it.
  Only when every door fails does the person see the plain failure line.
  Since #11040 the worker asks for zero retention (below), and the privacy
  answer (`meta.privacy`, `meta.data_retention`) says what the worker asks
  (`coder::first::keeps_sentence`), and
  "What model is this?" (`meta.model`) names the model answering now,
  Gemini while the primary's last turn failed before its first words. A
  turn the router sends to a retrieval or a CLI proposal holds the running
  model's words until that seam answers: Gemini's first token always came
  after the retrieval, and the primary's comes before it, so without the
  hold the first release answered "summarize your essay" turns ungrounded.
  The keys are in the worker's environment file on its host and nowhere else.
  No model API key ships in the app.
- **Embeddings on Vertex AI (2026-10-10, #11219).** The product knowledge
  base and the Gym records embed with Google's `gemini-embedding-001` at
  768 dimensions on Vertex AI (`OPENAGENTS_PRODUCT_KB_EMBEDDINGS=vertex`,
  `KB_VERTEX_PROJECT=openagentsgemini`, `KB_VERTEX_MODEL=gemini-embedding-001`,
  the same metadata-server token), re-embedding their corpus at startup, so
  no shipped vectors change. The codebase index ships rebuilt on
  `text-embedding-005` (`CODER_CODEBASE_EMBEDDINGS=vertex
  scripts/build-codebase-kb.sh`, 250 chunks a request; Gemini's model takes
  one input a request, too slow for 43,660 chunks), and the worker reads it
  with the model the index names, so an index and its questions never mix
  models (`coder::codebase::embedder_for`). An older OpenAI-built index
  still opens with the gateway embedder. Vertex has no second embedding
  provider of the same model, so a failed Vertex embedding costs that turn
  its retrieval, never its answer. Measured from the VM: a query embeds in
  0.13–0.16 s on `text-embedding-005` and 0.16–0.21 s on
  `gemini-embedding-001`. The web chat goldens' router mode
  (`chat-goldens router`, Jev live) passed 98 of 116 with
  `gemini-embedding-001` and 94 of 116 with `text-embedding-005`, agreeing
  on route, tier, and answer for 103 of 116, so the product notes use
  Gemini's model. A job on the caller's own keys (BYOK) embeds on their
  provider, whose model differs from these vectors, so its product-note
  retrieval is off for that job.
- **Embeddings and the judge fail over too.** The product knowledge base,
  the Gym records, and the codebase index embed with
  `openai/text-embedding-3-small` on the provider the configuration names,
  with every other provider of ours that has a key here behind it (OpenRouter,
  the Vercel AI Gateway, OpenAI: the same model, so the cache stays valid);
  a failed call goes to the next and logs `embeddings: … failed (…); …
  takes the call`. The AI Gateway embedder reads `AI_GATEWAY_API_KEY`
  first and `CODER_DOOR_KEY` only while the door is the gateway, so an
  OpenRouter door key is never sent to Vercel. Jev's judge already asked
  the Vercel AI Gateway, OpenRouter, then TypeSafe (`jev::doors`); on
  2026-10-09 it stopped at the first door's 402 because OpenRouter answered
  the alias `typesafe/jev-latest` with 400 "does not exist", which is not a
  failover. `jev-latest` now names the newest alias there
  (`typesafe/jev-1.13`), and a 404 fails over like a 402. A judge that
  still fails costs the turn its opener, never its answer.
- **What we ask the providers (#11040).** Every chat-model request carries
  `"store": false`. Under `CODER_PROVIDER_PRIVACY=strict` (the default when
  unset), OpenRouter requests (the primary and T1 personalization) also
  carry `provider: {"data_collection": "deny", "zdr": true}`, so OpenRouter
  routes them only to endpoints that neither train on nor keep the request,
  and Vercel AI Gateway requests carry
  `providerOptions: {"gateway": {"zeroDataRetention": true}}`. A model with
  no such endpoint is refused by the router, which the chat treats like any
  other primary failure: the turn goes to the fallback. `no-training` keeps
  only OpenRouter's `data_collection: "deny"`; `off` sends neither. The
  worker's `privacy` log line names the level, and an unknown value stops
  it. Check the journal after a deploy: if every turn now names Gemini,
  the primary has no zero-retention endpoint. Jev's doors and the
  embeddings calls do not send these fields yet.
- **First response and the chat router.** The worker acknowledges every
  admitted turn with `status: processing` at once. With `TYPESAFE_API_KEY`
  set, a turn that asks gets one Jev (System One) judgment run beside the
  model call, the `chat-router-v2` question set (`crates/coder/src/router/`):
  route, prepared answer, whether the reply needs the user's specifics,
  lane, opener, risk, and a command group when a command tree is wired.
  Code decides what is shown. A turn that sends `"router":
  "chat-router-v1"` (build 20 of the app) gets every tier: a whole answer
  from the reviewed bank (`crates/coder/answers/chat-answers-v1.toml`) with
  followup chips, a refusal, a "Working on …" stem with a Run
  Coder or Connect a computer offer, a wallet or account answer with its
  screen offered, or the model led by an opener. A turn that sends only
  `"opener": true` (builds 19 and earlier) gets a whole answer with no offer, an
  opener, or nothing, as before. Every judged turn logs one `router` line
  of ids, probabilities, tiers, and the judge's time, never message text.
  `CODER_WORKER_ROUTER=shadow` logs what the router would serve but serves
  what `opener` alone would; `off` ignores `router`; unset is `live`. The
  worker refuses to start when the bank breaks its lint. See the
  [chat router design](../coder/design/2026-09-28-chat-router.md) and
  [`docs/coder/measurements/2026-09-28-first-reply.md`](../coder/measurements/2026-09-28-first-reply.md).
  The startup line names the question set and the bank by digest
  (`router chat-router-v2@<digest> (Live), bank chat-answers-v1@<digest>`),
  and every judgment carries the same two identities in `set` and `bank`:
  they are what a router eval report pins as the subject's configuration
  ([#9959](https://github.com/OpenAgentsInc/openagents/issues/9959),
  [`docs/coder/measurements/2026-09-29-chat-router-claims.md`](../coder/measurements/2026-09-29-chat-router-claims.md)).
- **Jev's doors.** The judge asks the Vercel AI Gateway's
  TypeSafe-compatible API first (`typesafe-ai/jev`, under
  `AI_GATEWAY_API_KEY`; the gateway routes Jev to TypeSafe itself, with the
  owner's key as its own fallback), then OpenRouter's Decisions API
  (`typesafe/jev-1.13`, under `OPENROUTER_API_KEY`), then TypeSafe direct
  (`TYPESAFE_API_KEY`) last (`jev::doors`, `Failover::primary_last`,
  `jev_hosted::resolve_with_fallbacks`; since #10110). A door is left for
  the next only for its own reasons (402 no credits, 401, 429, a 5xx, no
  connection, or no answer within the first door's share, three fifths of
  the 2.5 s first budget); a refusal of the question never fails over. A
  door that answered 401 or 402 is skipped for five minutes
  (`jev::doors::BENCH`) and then asked again; the journal logs the bench
  once (`a door refused for its key or account`). A door with no key is
  off, and the startup log names the order and which are on (`judge
  doors https://ai-gateway.vercel.sh → https://openrouter.ai →
  https://api.typesafe.ai (jev-latest)`, `judge   door
  https://openrouter.ai under $OPENROUTER_API_KEY`). The judgment the phone
  gets names the door that answered and the model it served (`door`,
  `model`), the thread's `openagents.decision-call.v1` record keeps them
  (`service.upstream`), the journal logs `judge answered by door …` when a
  door other than TypeSafe did, and the privacy answer names the doors that
  are on. The question set and its digest are unchanged, so the router's
  calibration stands. The [decision worker](decision-worker.md#the-doors-in-order)'s
  open lane asks in the same order since #10112.
- **A late judgment still routes (#10110).** On a turn that asks for a
  first response, the model starts at once but its words wait for the
  judgment. A healthy judge answers in about a quarter second, so nothing
  changes then. Past the first budget (2.5 s, `first::BUDGET`) the bank's
  `explain` opener ("We'll look that up for you.") shows as partial `seq` 0
  while the judgment finishes, and the routed reply follows it; past the
  second bound (6 s, `first::LATE`) the model's reply goes out unrouted.
  That call always carries a fixed note (`first::UNROUTED_NOTE`): the Gym,
  the Verse, Coder, plugins, and Jev are ours, so answer about ours and do
  not offer Coder for a question. The journal logs `judge ran past 2500 ms;
  holding the model up to 6000 ms` and, when the judgment never comes,
  `judge ran past 6000 ms; the model answers unrouted`.
- **Router calibration.** `CODER_WORKER_ROUTER_CALIBRATION=on` runs each
  judgment's `route` and `answer` probabilities through the calibration
  maps the last published eval fitted
  (`crates/coder/fixtures/chat-router/calibration-v2.json`, one reliability
  table per question, fitted on the labeled set's calibration partition)
  before the policy table decides; the `router` log line then names the
  map (`calibration-v2@<digest>`). Unset or `off` (the default) serves the
  raw probabilities, which the policy's thresholds were tuned on. The
  worker refuses to start with it on when the committed map was fitted for
  another question set than this build asks (rerun the published eval to
  refit), and logs when the bank has moved since the fit. The maps change
  only probabilities, never which route or answer was chosen.
- **Personalization (T1).** The chat router's stems ("Working on …",
  "Looking through …", "Picking up …") are finished by a cheap model through `coder::router::personalize`;
  the worker reads `personalize::seam_from_env()` at start, and its
  `router` line names whether personalization is on. It is off unless the
  worker's environment sets it:
  - `CODER_PERSONALIZE=openrouter` turns it on with OpenRouter's
    `google/gemini-2.5-flash-lite`, the measured choice
    ([the chat router design](../coder/design/2026-09-28-chat-router.md#implemented-and-measured-2026-09-28));
    `openrouter:<model>` names another model. `gateway` (or
    `gateway:<lane>`) uses the door key and URL the worker already has and
    the `glm` lane, about twice as slow; that lane writes only the stem's
    ending, not the chat's answer, which stays on Space Bunny Alpha then
    Gemini. `off` or unset turns it off. The shipped chat environment sets
    `openrouter`.
  - With `openrouter`, `OPENROUTER_API_KEY` is required in the chat
    environment file (`/etc/coder-worker/coder-worker-chat.env`), beside the
    gateway key: the owner's OpenRouter key, which the worker never logs.
    The OpenRouter account must have credits; without them every
    personalized stem closes with its generic ending. `seam_from_env()`
    refuses to start without a key rather than run without it.
  - With it on, each personalized turn sends the route, the stem, and the
    user's latest message (redacted of secret shapes, at most 600
    characters) to that provider, so the privacy answer and the Basic chat
    rows in `INVARIANTS.md` must name it as a recipient (the seam's
    `recipients()`) in the change that turns it on.
- **Product knowledge.** The router's `product.kb` turns answer from
  `knowledge/openagents/` through `coder::product_kb` when three things are
  present at startup: the judge, the corpus, and an embeddings key. The
  corpus is read from `OPENAGENTS_PRODUCT_KNOWLEDGE`, or from the checkout
  the binary was built in, which does not exist on the VM, so copy
  `knowledge/openagents/` beside the release and set the variable. The
  embeddings key is `OPENAI_API_KEY` (or `~/.openagents/openai.json`), else
  OpenRouter's; `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=vertex` uses Vertex AI
  instead. The log says `product kb openagents-product@… (52 entries),
  embeddings through OpenAI`, or `product kb off:` and why. With it on, a
  product turn's latest message also reaches that embeddings provider,
  which the seam names in `recipients()`. See
  [the measurement](../coder/measurements/2026-09-28-product-kb.md).
  `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway` embeds through the Vercel AI
  Gateway with the door key the worker already has; the deployed worker
  uses it.
- **Codebase knowledge.** `codebase.kb` turns answer from an index of this
  repository at a pinned commit (`coder::codebase`,
  [codebase-kb.md](../coder/design/codebase-kb.md)), read from
  `CODER_CODEBASE_KB`. Build it on a development Mac with
  `scripts/build-codebase-kb.sh <COMMIT> <OUT>` (the door key embeds it
  through the gateway; a refresh re-embeds only changed chunks) and copy it
  beside the release. The worker keeps the embedder's connection warm.
- **Gym records.** The `gym.news` and `eval.*` routes answer from the
  Gym's verified records (`coder::gym_kb`) when the judge, the product
  corpus, and the product knowledge base's embeddings are all present; it
  needs no variable of its own. Its tool catalog and Gym notes are the
  product corpus's entries tagged `tool` and `gym`, its builds come from
  the app's changelog compiled into the binary, and published results are
  read from the worker's own relay every 10 minutes and admitted only when
  `nostr::eval_ext` verifies them. The same read takes the starter test
  sets, the hosted runner's `3184` releases, and fetches their files by
  digest from the runner's public bucket
  (`https://storage.googleapis.com/openagentsgemini-eval-blobs`;
  `CODER_EVAL_BLOBS` names another, `off` reads none), so `eval.run` offers
  **Start the test** on the hosted runner before anyone has published a
  result (#9943). The log says `gym records: 3 tools, 3 builds, … notes`
  and then `gym records: N verified results, T test sets, M refused` after
  each read, or `gym records off:` and why. A `gym.news` reply's
  citations are taken out before the phone sees them, and each grounded
  reply logs `router gym reply:` with its citation counts and the
  post-check's banned words and raw-id count (#9944), and when the
  records, the model's first words, and the end arrived. A grounded news
  reply opens with the bank's `gym.news.lead` line, sent with the news
  card as soon as the records are judged, and runs on the primary first
  and then on Gemini 2.5 Flash with its reasoning off through the same
  gateway and key (#9950, #10109);
  `CODER_GYM_NEWS_MODEL` names another model, and `off` keeps news on the
  chat model. The log says `gym news google/gemini-2.5-flash with its
  reasoning off`, or with a primary `gym news stealth/space-bunny-alpha
  first, then google/gemini-2.5-flash with its reasoning off`. A request may name
  `chat-router-v1` (build 20) or `chat-router-v2`; both are routed with
  v2. The authoring interview (`eval.author`, `coder::eval_author`) runs
  on the worker's door (the primary first) and judge; without them it answers with the
  bank's `eval.author.soon`.
- **CLI route.** With a judge, the worker holds `coder::cli_route`'s
  `CommandRoute` as its CLI seam, filling free text through its own door;
  `CODER_WORKER_CLI=off` turns it off. On the phone it proposes only the
  owner's read-only list, which the phone runs itself (`computer list`,
  `show`, `workspaces`) or on the connected computer as `openagents --json`
  through NIP-HOST `terminal.open`, after a tap.
- **Streaming.** The worker sends its first delta at once and then about 160
  bytes at a time; the phone checks each answer's signer, recipient, request
  binding, and sequence, draws the reply with Rust Native's incremental
  Markdown, and replaces the preview with the result.

- **Liveness.** The worker never trusts a quiet socket
  ([#9946](https://github.com/OpenAgentsInc/openagents/issues/9946)).
  `relay.openagents.com` is a Cloud Run domain mapping, so the worker's
  WebSocket ends at Google's front end, and when the relay instance behind
  it restarts, the front end can keep the worker's TCP connection
  established while nothing reaches it. On 2026-09-29 the worker read
  silence that way for 40 minutes, and every phone request went
  unanswered. Now the worker sends a probe every 30 seconds (a `REQ` with
  `limit` 0 on the jobs filter, which the relay answers with `EOSE` at
  once) and treats a probe still unanswered at the next one as a dropped
  connection: it logs `relay: the relay stopped answering: …;
  reconnecting in 1 s` and subscribes again. Every 45 minutes, before the
  relay's one-hour request timeout, it opens a second connection,
  subscribes there, and only then closes the first (`renewed the jobs
  subscription on a new connection`), so no job falls into the gap.
  `CODER_WORKER_PROBE_MS` and `CODER_WORKER_RENEW_MS` change the two
  periods. Each answered probe also sends systemd `WATCHDOG=1`; the unit's
  `WatchdogSec=120` restarts a worker that stops proving its subscription.

The alternatives were a new HTTPS endpoint with its own device
authentication, or the decision-API gateway (`crates/gateway`). Both need a
new credential flow for the phone. The relay door already authenticates by
the device key, already streams, and already has a deployed worker and
runbook, so OpenAgents chat reuses it.

## Through the inference gateway

Since #11064 the worker can send every model call through the OpenAgents
inference gateway ([docs/inference/gateway.md](../inference/gateway.md))
instead of calling OpenRouter and the Vercel AI Gateway itself. The
gateway's router picks the model and upstream (prepaid Google credit on
Vertex first for `openagents/chat`, then the Pro door, then OpenRouter),
falls back before the first token, and records every attempt.

| Variable | Meaning |
| --- | --- |
| `CODER_INFERENCE_KEY` | The gateway's `oak_` service key. Set, the switch is on. |
| `CODER_INFERENCE_URL` | The gateway's base URL; default `http://127.0.0.1:8790`. |
| `CODER_INFERENCE_MODEL` | Model id or task class to ask for; default `openagents/chat`. |
| `CODER_WORKER_INFERENCE` | `gateway` or `direct`. Unset: `gateway` when the key is set. `direct` keeps the OpenRouter primary and the Vercel lane described above. |

In gateway mode the worker needs no provider key for generation; Jev's
doors, personalization, and product search still read theirs. The Gym news
lane asks for `openagents/fast` unless `CODER_GYM_NEWS_MODEL` names a
model. Each result names the model the gateway chose, and the usage log's
`door` reads `<upstream> via <gateway host>`.

Locally, `scripts/dev/inference-local.sh` starts the gateway, a worker from
this checkout on a fresh key, and the website on `127.0.0.1:4300` pointed at
it. The deployed worker keeps the direct doors until the gateway is
deployed and its env file gets a service key.

## Admission: no usage limits

The owner decided on 2026-10-01
([#10120](https://github.com/OpenAgentsInc/openagents/issues/10120)): no
usage limit anywhere in the app. The chat worker has to answer a key nobody
has seen, so it runs open (`CODER_WORKER_OPEN=1`) and counts nothing.
Instead, it records every job in its usage log
([chat-worker-usage.md](chat-worker-usage.md)).

| Bound | Deployed value | Refusal |
| --- | --- | --- |
| Jobs per caller key, per minute or per day | None | Never |
| Jobs for every caller together per day | None | Never |
| Request ciphertext | 96 KiB, the size of a request a relay event holds | `limit_exceeded` ("This conversation is too long for us to answer here. Start a new chat.") |
| Jobs at once | 64 | `busy` ("We're busy right now. Try again in a moment.") |

- An open caller gets conversation jobs only: a delegation is refused
  `not_admitted`, and execution requests are ignored. Keys on
  `CODER_WORKER_ALLOW` (the owner's) get everything.
- `CODER_WORKER_QUOTA` is an abuse brake for emergencies only, off in the
  shipped configuration and in `deploy/coder-worker-chat.env.example`.
  Unset is unlimited, and each part is unlimited unless named:
  `minute=N`, `day=N` (per caller key), `total=N` (every caller together
  per UTC day), `bytes=N` (the size bound). Setting it also opens the
  worker. Its refusals are `rate_limited` and `quota_exhausted`; the day's
  counts go to `CODER_WORKER_QUOTA_FILE` when a day count is set.
- No surface shows a limit. The phone, the desktop app, the terminal, and
  the website show an old worker's `rate_limited` or `quota_exhausted` as
  "Couldn't reach OpenAgents; try again." (`basic_coder::Failure::describe`).
- The phone sends at most the newest 48 KiB of the conversation, so it stays
  inside the size bound.

`INVARIANTS.md` (Basic chat) records these as invariants with their tests.

## Deploying

The chat worker runs beside the executor worker on the Coder worker VM
(`oa-coder-worker-1`), under its own unit and environment file. Follow
[`deploy/README.md`](../../deploy/README.md) for the user and release
directory. The chat worker gets its own release symlink,
`/opt/coder-worker/chat`, through a drop-in, so upgrading it never changes
the binary the executor worker runs from `current`.

The VM has no Rust toolchain or C compiler. Build a static Linux binary on a
development Mac with `cargo-zigbuild` (the target needs only
`rustup target add x86_64-unknown-linux-musl`), then copy it, the filled
environment file, the unit, and the drop-in to the VM with
`gcloud compute scp --tunnel-through-iap` into a private directory (not
`/tmp`, where another user's `coder-worker` file already sits):

```sh
cargo zigbuild --locked --release -p coder --bin coder-worker \
  --target x86_64-unknown-linux-musl
```

On the VM, with `<VERSION>` the short commit:

```sh
sudo install -d -o root -g root -m 0755 /opt/coder-worker/releases/<VERSION>
sudo install -o root -g root -m 0755 coder-worker /opt/coder-worker/releases/<VERSION>/coder-worker
sudo ln -sfn /opt/coder-worker/releases/<VERSION> /opt/coder-worker/chat
# The filled env: the worker secret, the gateway key, and TYPESAFE_API_KEY.
# systemd reads it as root, so it can be root-only.
sudo install -o root -g root -m 0600 coder-worker-chat.env /etc/coder-worker/coder-worker-chat.env
sudo systemd-run --pipe --wait -p User=coder-worker \
  -p EnvironmentFile=/etc/coder-worker/coder-worker-chat.env \
  /opt/coder-worker/chat/coder-worker --check
sudo install -o root -g root -m 0644 deploy/systemd/coder-worker-chat.service \
  /etc/systemd/system/coder-worker-chat.service
sudo install -d -m 0755 /etc/systemd/system/coder-worker-chat.service.d
sudo tee /etc/systemd/system/coder-worker-chat.service.d/release.conf >/dev/null <<'EOF'
[Service]
ExecStartPre=
ExecStartPre=/usr/bin/test -x /opt/coder-worker/chat/coder-worker
ExecStart=
ExecStart=/opt/coder-worker/chat/coder-worker
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now coder-worker-chat
sudo journalctl -u coder-worker-chat -n 20 --no-pager
```

To upgrade, install the new release beside the old one, move the `chat`
symlink, and `sudo systemctl restart coder-worker-chat`.

The unit in `deploy/systemd/coder-worker-chat.service` carries
`WatchdogSec=120` and `NotifyAccess=main`; a worker built before #9946
sends no watchdog notifications, so install the unit only with a release
that does, or systemd restarts it every two minutes.

The first log line must be `worker  32c07895…` (the key the app carries),
the judge line must name the doors in order, the Vercel AI Gateway first and
`https://api.typesafe.ai` last, the admits line must say `every caller with no usage
limit`, and the usage line must name the log's directory. The worker secret is kept with the
owner's secrets as `coder-chat-worker.env`; the gateway key is the owner's
AI Gateway key.

To check the deployed path, send one turn from a fresh, throwaway key, as a
new install does, with the `openagents` command
([guide](../cli/chat.md)):

```sh
openagents chat --scratch --json "How do I connect a phone"
openagents chat --scratch --json "Write a haiku about rain"
```

Each prints NDJSON: `accepted`, the streamed `partial`s, the router's
`route` (tier, route, bank, served answer, and the judgment as it arrived),
any `offer`, and the `result` with the model the worker named, or a
`failure` with the chat's own words; the exit code is 0 only for a result.
The first should be the product-knowledge answer about the QR code, with no
`[openagents.` tags. `--scratch` never touches a real identity, store, or
host. `live_basic_coder_streams_a_reply` in `crates/openagents-chat` (run
through `crates/openagents-mobile`) still measures time to first words and
the legacy request shape.

### When the phone gets no answer

"We couldn't reply this time" for every message, with the worker's unit
still `active`, means the worker is not receiving jobs. Check in this
order on `oa-coder-worker-1`:

1. `sudo journalctl -u coder-worker-chat --since -15min --no-pager`. A
   healthy worker logs nothing between jobs except `gym records` every 10
   minutes and `renewed the jobs subscription` every 45 minutes. `relay:
   … reconnecting` lines mean the relay is failing and the worker is
   rejoining it; check the relay (`openagents-nostr-relay` on Cloud Run,
   [runbook-cloud-run.md](runbook-cloud-run.md)).
2. `systemctl show coder-worker-chat -p WatchdogTimestamp -p NRestarts`.
   A `WatchdogTimestamp` that is not recent, or restarts climbing, means
   the worker cannot prove its subscription.
3. Run `openagents chat --scratch --json "Write a haiku about rain"`
   (above) from a checkout. If it
   fails while the journal shows no fault, save `sudo ss -tnpi` for the
   worker's PID (the relay socket's `lastrcv` says how long it has heard
   nothing) and the journal, then `sudo systemctl restart
   coder-worker-chat` and open an issue with both.

On 2026-09-28 that test answered through the production relay in 5.7 s,
with the first words at 5.1 s, from a worker with this configuration running
on a development machine.

After the production deploy on `oa-coder-worker-1` (release `358975bdbd`,
2026-09-28), three runs of the same test showed the first words (the Jev
opener) at 0.50 to 0.59 s and the finished reply at 4.9 to 5.3 s. `chat-load-bench --basic-coder 5`, which does
not ask for an opener, measured Send to first words at a median 4.2 s (the
model's first token) and Send to done at a median 4.4 s.

Release `9ca003acbe` (2026-09-28) replaced the first response with
`coder-first-response-v2`: prepared answers in the plural OpenAgents voice,
no filler openers, and confidence floors. It was built with `cargo zigbuild`
as above, installed as `/opt/coder-worker/releases/9ca003acbe`, checked with
`--check` under the chat environment, and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; `coder-worker.service` and
`/opt/coder-worker/current` were not touched. `358975bdbd` stays in
`releases/` for rollback (move the symlink back and restart). With the app
asking `"opener": true` again, `live_basic_coder_streams_a_reply` answered
"Who are you?" with the whole `meta.who` answer in 0.52 s, "hi" in 0.46 s,
and "What model are you?" in 0.62 s (result `model: bank:chat-answers-v1`,
no model call); its default Nostr question showed "Here's how that works."
at 0.57 to 0.96 s and finished at 5.2 to 6.3 s; "Connect to my GitHub" was
left to the model (first words 3.2 s) with `lane: computer`. To check a
particular message, set `OPENAGENTS_TEST_CHAT_MESSAGE`.

Release `b8dc0057fb` (2026-09-28) put the chat router live with every seam:
the structured `chat-router-v1` questions and bank `chat-answers-v1@df722f59fb54`,
T1 personalization (`CODER_PERSONALIZE=openrouter` and `OPENROUTER_API_KEY`
added to the root-only environment file), product knowledge
(`knowledge/openagents/` copied to `/opt/coder-worker/releases/b8dc0057fb/knowledge/openagents`,
`OPENAGENTS_PRODUCT_KNOWLEDGE=/opt/coder-worker/chat/knowledge/openagents`,
`OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway`), codebase knowledge (an index at
`75cecb8dba` copied to `/opt/coder-worker/releases/b8dc0057fb/codebase-kb.gz`,
`CODER_CODEBASE_KB=/opt/coder-worker/chat/codebase-kb.gz`), and the CLI
route. Copy the corpus with `COPYFILE_DISABLE=1` or delete `._*` files on the
VM: a macOS tar's AppleDouble files make the corpus refuse to load. The
previous environment file is kept as `coder-worker-chat.env.bak-9ca003acbe`,
and `9ca003acbe` stays in `releases/` for rollback (restore that file, move
the symlink back, restart). `coder-worker.service` and
`/opt/coder-worker/current` were not touched. The log names
`product kb openagents-product@841bda9956e9 (52 entries), embeddings through
the Vercel AI Gateway (OpenAI embeddings)` and `seams Seams { personalize:
true, product: true, codebase: true, cli_groups: 29 }`. Live timings and
tiers, including a build-19 request (`OPENAGENTS_TEST_CHAT_LEGACY=1`), are in
[the tuning measurement](../coder/measurements/2026-09-28-chat-router-tuning.md#live-on-the-deployed-worker).

Release `50eb35b8b7` (2026-09-28) carries the #9928 retune (bank
`chat-answers-v1@ab93b7cdda51`). It was built and checked as above,
installed as `/opt/coder-worker/releases/50eb35b8b7` with `knowledge/` and
`codebase-kb.gz` copied from `b8dc0057fb`, and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `b8dc0057fb` stays in
`releases/` for rollback. The live check is in
[the tuning measurement](../coder/measurements/2026-09-28-chat-router-tuning.md#retune-for-9928-capability-questions-and-who-built-this).

Release `17c7484f9f` (2026-09-29) puts `chat-router-v2` live
([#9936](https://github.com/OpenAgentsInc/openagents/issues/9936)): the Gym
and eval routes, the Gym's records, cards, and eval offers, and the
authoring interview (#9937), with bank `chat-answers-v1@97cf64318f8f` (53
answers). It was built as above, installed as
`/opt/coder-worker/releases/17c7484f9f` with the current
`knowledge/openagents/` (60 entries; copied with `COPYFILE_DISABLE=1`) and
`codebase-kb.gz` from `50eb35b8b7`, checked with `--check` under the chat
environment, and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file did not change,
`coder-worker.service` and `/opt/coder-worker/current` were not touched,
and `50eb35b8b7` stays in `releases/` for rollback. The log names
`gym records: 3 tools, 3 builds, 6 notes`, `seams Seams { …, gym_tools: 3,
author: true }`, and `gym records: 0 verified results, 0 refused` after the
first relay read (nothing is published yet). The live check is in
[the chat-router-v2 measurement](../coder/measurements/2026-09-29-chat-router-v2.md#live-on-the-deployed-worker).


Release `ebaa2af04a` (2026-09-29) carries the #9945 fix (a tool made in
chat is a skill unless Jev is sure it needs new code, and the router sends
"write tests for Project map" to the interview) on top of `5882b3910e`
(the #9946 subscription probe). It was built as above, installed as
`/opt/coder-worker/releases/ebaa2af04a` with `knowledge/` and
`codebase-kb.gz` copied from `5882b3910e`, checked with `--check` under the
chat environment, and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change, and
`5882b3910e` stays in `releases/` for rollback. With
`live_basic_coder_streams_a_reply` and `OPENAGENTS_TEST_CHAT_MESSAGE`, "Help
me make a tool that writes changelog entries", "… tells Coder how we write
commit messages", and "… writes docstrings for new functions" each reached
the step-1 draft ("Is that the tool? Tap Looks good…") in 3.4 to 5.3 s;
"Help me write tests for Project map" routed to `eval.author` and described
Project map at step 1; "make a tool that sends me a Telegram message when
Coder finishes a task" got the Run Coder offer in 0.8 s.

Release `0546032e17` (2026-09-29) reads the starter test sets, so
`eval.run` offers **Start the test** on the hosted runner (#9943), and
keeps citation ids and banned words off Gym news (#9944), on top of
`ebaa2af04a` (#9945) and `5882b3910e` (#9946). It was built as above,
installed as `/opt/coder-worker/releases/0546032e17` with the current
`knowledge/openagents/` (copied with `COPYFILE_DISABLE=1`, `._*` removed;
the `openagents.gym-results` note changed) and `codebase-kb.gz` from
`ebaa2af04a`, checked with `--check` under the chat environment, and put
live by moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, and `ebaa2af04a` stays in
`releases/` for rollback. The first read logged `gym records: 14 verified
results, 4 test sets, 0 refused`. With `live_basic_coder_streams_a_reply`,
"Test Project map on Coder" answered in 0.60 s with the tool card and a
`start_eval` for the newest Project map test set (`8e4ae48b…`, 6 tests, 3
runs, 2 arms, `hosted`, the starter catalog's `repo-map` reference); "What's
new in the Gym?" answered from five records in 7.6 s (first words 6.9 s)
with no citation id and no banned word, and the worker logged `router gym
reply: 3 cited, 0 invented, banned [], 0 raw ids`.

Releases `c975123ca2`, `8fdad245cc`, and `a9ba87d828` (2026-09-29, the
build 22 verification) change which results Check a result offers: only
our catalog tools' results
([#9951](https://github.com/OpenAgentsInc/openagents/issues/9951)), and
only those run under the newest subject lock read for their test set, so a
check of one still earns XP after a runner redeploy
([#9952](https://github.com/OpenAgentsInc/openagents/issues/9952));
`a9ba87d828` also carries build 22's changelog for Gym news. Each was built
and installed as above with `knowledge/` and `codebase-kb.gz` copied from
`dbad257c51`, checked with `--check`, and put live by moving the `chat`
symlink; the environment file and unit did not change.
`live_basic_coder_streams_a_reply` passed on `a9ba87d828` (first words
0.68 s). See
[the build 22 verification](../extensions/measurements/2026-09-29-build-22-verification.md).

Release `af6d0fae2d` (2026-09-29) carries build 23's changelog ("Ready for
launch": no sample screens, no empty playtest card, plain reasons when we
can't run your tests) for Gym news; the worker's code did not change since
`a9ba87d828`. It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/af6d0fae2d` with `knowledge/` (no `._*` files)
and `codebase-kb.gz` copied from `a9ba87d828`, checked with `--check` under
the chat environment, and put live by moving the `chat` symlink and
restarting `coder-worker-chat`; the environment file and unit did not
change, `coder-worker.service` and `/opt/coder-worker/current` were not
touched, and `a9ba87d828` stays in `releases/` for rollback. The first read
logged `gym records: 28 verified results, 4 test sets, 1 refused`.
`live_basic_coder_streams_a_reply` passed (first words 0.68 s, done
6.3 s). "What's new in the Gym?" (three runs) showed first words at 1.2 to
1.9 s and finished at 2.9 to 3.6 s, with a `Build 23: Ready for launch`
news card and replies naming build 23's items; the worker logged `router
gym reply: … 0 invented, banned [], 0 raw ids` for each. "Who are you?"
answered with the whole `meta.who` answer in 0.66 s, and "Test Project map
on Coder" answered in 0.91 s with the tool card and a `start_eval` for the
newest Project map test set (6 tests, 3 runs, 2 arms, `hosted`).

Release `dbad257c51` (2026-09-29) makes Gym news answer at once (#9950),
on top of `d0a053650d` and `69587cc286` (phone only). It was built as
above, installed as `/opt/coder-worker/releases/dbad257c51` with the
current `knowledge/openagents/` (copied with `COPYFILE_DISABLE=1`, `._*`
removed; unchanged from `d0a053650d`) and `codebase-kb.gz` from
`d0a053650d`, checked with `--check` under the chat environment, and put
live by moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, and `d0a053650d` stays in
`releases/` for rollback. Over ten fresh-key runs of "What's new in the
Gym?", the first words went from a median 8.1 s (p90 11.0 s) to 1.15 s
(p90 1.46 s) and the whole reply from 9.0 s to 2.5 s, every reply with no
invented citation, banned word, or raw id
([the measurement](../coder/measurements/2026-09-29-gym-news-latency.md)).

Release `5882b3910e` (2026-09-29) adds the liveness probes, the overlapping
renewal, and the systemd watchdog
([#9946](https://github.com/OpenAgentsInc/openagents/issues/9946)). Before
it, at 07:34 UTC, the worker was deaf a second time that morning: the
relay's instance had restarted at 07:13 after losing its Postgres
notification listener, the relay's request log ended the worker's
WebSocket then, and `ss` on the VM still showed the worker's socket to the
front end `ESTABLISHED` with nothing received for 36 minutes. A restart
fixed it, as before. The release was built as above, installed with
`knowledge/` and `codebase-kb.gz` copied from `17c7484f9f`, checked with
`--check`, and put live with the updated unit (`WatchdogSec=120`,
`NotifyAccess=main`); `coder-worker.service` and `/opt/coder-worker/current`
were not touched. The log names `liveness a probe every 30 s; the
subscription is renewed every 2700 s`, and `systemctl show -p
WatchdogTimestamp` advances every 30 seconds. `live_basic_coder_streams_a_reply`
passed 11 times out of 11 over the next 31 minutes (one every 3 minutes),
with first words at 0.57 to 0.67 s, across two later releases
(`ebaa2af04a`, `0546032e17`) that another change deployed on top of it
during the run.
The first production renewal ran on schedule at 08:42:09, 45 minutes after
the `0546032e17` start (`renewed the jobs subscription on a new
connection`, no `relay:` fault, `NRestarts=0`), and the live test answered
through the renewed connection in 0.65 s.

Release `375cef66ef` (2026-09-29) puts `chat-router-v3` live: the
missing-capability route and its `capability` card over the admitted
capability set ([#9960](https://github.com/OpenAgentsInc/openagents/issues/9960)),
the question-set and bank digests on the wire, the router evidence record
and the calibration switch (off) ([#9959](https://github.com/OpenAgentsInc/openagents/issues/9959)),
and the "capability" vocabulary in the Gym notes and replies
([#9957](https://github.com/OpenAgentsInc/openagents/issues/9957),
[#9958](https://github.com/OpenAgentsInc/openagents/issues/9958)). It was
built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/375cef66ef` with the current
`knowledge/openagents/` (61 entries, seven Gym notes changed; copied with
`COPYFILE_DISABLE=1`, no `._*` files) and `codebase-kb.gz` from
`af6d0fae2d`, checked with `--check` under the chat environment ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; the environment file and unit
did not change, `coder-worker.service` and `/opt/coder-worker/current` were
not touched, and `af6d0fae2d` stays in `releases/` for rollback. The log
names `router chat-router-v3@a19f8c201312 (Live), bank
chat-answers-v1@a04859020455 with 58 answers`, `calibration off: raw
probabilities`, and `gym records: 34 verified results, 7 test sets, 1
refused` on the first read. With
`live_basic_coder_streams_a_reply`, "Book me a flight to Austin tomorrow"
answered in 0.71 s (first words 0.68 s) from the bank with route
`capability.missing`, answer `capability.missing@1`, and the card "There's
no capability for that yet. Anyone can add one and test it in the Gym, so
everyone can see whether it helps."; "Who are you?" still answered from the
bank. The record is
[the build 25 record](../extensions/measurements/2026-09-29-build-25-chat-first.md).

Release `3691fb2e1c` (2026-09-29) fixes the answer to "How do I connect a
phone" ([#9995](https://github.com/OpenAgentsInc/openagents/issues/9995)):
the product knowledge entries describe QR pairing with OpenAgents for Mac
instead of the removed Tailscale and eight-character-code setup, and a
grounded product reply's `[openagents.…]` citations are logged (`router
product reply: cited […], unknown […]`) and taken out of every streamed
piece and the final text. It was built with `cargo zigbuild` as above,
installed as `/opt/coder-worker/releases/3691fb2e1c` with the current
`knowledge/openagents/` (60 entries; copied with `COPYFILE_DISABLE=1`, no
`._*` files) and `codebase-kb.gz` from `375cef66ef`, checked with `--check`
under the chat environment ("the configuration is safe to deploy"), and put
live by moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `375cef66ef` stays in
`releases/` for rollback. (`223a2c5a85` was live for a few minutes with a
route-description edit the live router does not read, then the symlink went
back to `3691fb2e1c`.) The log names `product kb
openagents-product@0ced76d89147 (60 entries)`. With
`live_basic_coder_streams_a_reply` and `OPENAGENTS_TEST_CHAT_MESSAGE`:

- "How do I connect a phone" answered in 1.09 s from the knowledge base
  (tier `canned`, answer `openagents.connect-computer@2`): "Open OpenAgents
  for Mac and it shows a QR code. Scan it with your iPhone Camera, or in our
  app tap Account, Computers, Connect a computer and point it at the code.
  Both screens then say the computer is connected. No Tailscale and no
  commands. On the same Wi-Fi, the Mac can also show up under Nearby: tap
  it, check both screens show the same six-digit code, and click Connect on
  the Mac. Can't scan? Click Copy a code instead and paste it in the app."
- "Do I need Tailscale to connect my Mac?" answered from
  `openagents.tailnet@2`: "No, you don't need Tailscale. …". A bare "Do I
  need Tailscale?" is still judged `general` by the router and the model
  explains Tailscale in general, with no OpenAgents steps; widening the
  `product.kb` route in `router/rubric.rs` moves the question set's digest
  and needs a new calibration run, so it is a follow-up.
- "How do I connect a Linux server that has no screen to the app?" was a
  grounded model reply naming `openagents connect invite` and `openagents
  connect --ssh HOST`; the worker logged `router product reply: cited
  ["openagents.cli"], unknown []`, and the reply showed no tag.

No reply contained `[openagents.`, `coder link`, `coder pair`, or an
eight-character code.

Release `7ae2a4dd41` (2026-09-30) routes a bare "Do I need Tailscale?" to
product knowledge ([#9997](https://github.com/OpenAgentsInc/openagents/issues/9997)):
the `product.kb` rubric in `router/rubric.rs` covers connecting a phone or
a computer and what connecting needs, Tailscale included, which moves the
question set to `chat-router-v3@c86d4a2ebeb2` with a refit
`calibration-v2.json` (held-out route accuracy 0.889 before and after;
[the measurement](../coder/measurements/2026-09-30-product-kb-connect-needs.md)).
It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/7ae2a4dd41` with the current
`knowledge/openagents/` (60 entries, the same files as `3691fb2e1c`;
copied with `COPYFILE_DISABLE=1`, no `._*` files) and `codebase-kb.gz`
from `3691fb2e1c`, checked with `--check` under the chat environment ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; the environment file and unit
did not change, `coder-worker.service` and `/opt/coder-worker/current`
were not touched, and `3691fb2e1c` stays in `releases/` for rollback. The
log names `router chat-router-v3@c86d4a2ebeb2 (Live), bank
chat-answers-v1@a04859020455 with 58 answers` and `calibration off: raw
probabilities`. With `live_basic_coder_streams_a_reply` and
`OPENAGENTS_TEST_CHAT_MESSAGE`:

- "Do I need Tailscale?" answered in 1.05 s from the knowledge base (route
  `product.kb` at 0.98, answer `openagents.tailnet@2`): "No, you don't
  need Tailscale. Your phone connects to your computer by scanning the QR
  code in OpenAgents for Mac, …".
- "Do I need Tailscale to connect my Mac?" answered in 1.26 s with the
  same `openagents.tailnet@2` answer (route `product.kb` at 1.0).
- "How do I connect a phone" answered in 1.15 s from
  `openagents.connect-computer@2` (route `product.kb` at 0.97): "Open
  OpenAgents for Mac and it shows a QR code. …".
- "What is Tailscale's pricing?" stayed `general` (0.95) and the model
  answered with Tailscale's plans; "Write a haiku about rain" stayed
  `general` (1.0) with the "Here's a draft." opener and a haiku.

No reply contained `[openagents.`.

TestFlight build 33 (1.0.0, archived from `3406cb3c49`, 2026-09-30)
changes no worker release. It is the first build whose hosted chat
transport, router fields, lifecycle, and encrypted chat cache come from the
shared `crates/openagents-chat` crate (`50faac3392`) instead of
`openagents-mobile`. On a fresh iPhone 17 Pro simulator (iOS 26.5, deleted
afterwards) against the live chat worker (`7ae2a4dd41`): "Who are you"
answered "We are OpenAgents. …"; "How do I connect a phone" answered
"Open OpenAgents for Mac and it shows a QR code. …" with the QR-code steps and no
`[openagents.` tag, showing Working with a stop button before the reply; a
model reply ("Explain in three short paragraphs how HTTP caching works")
arrived piece by piece; and after the app was quit and relaunched, the
chat was in Chats and opened with both replies. Build 33 was uploaded
with `build.sh upload` at 2026-09-29T22:17:14-07:00 and is `VALID`, in
Internal Testers (`IN_BETA_TESTING`).

TestFlight build 34 (1.0.0, archived from `270688fabe`, 2026-09-30)
changes no worker release. Since build 33 the phone's chat controller, chat
state and transcript projection, router offers and chat cards, chat
management, the encrypted chat store binding, and Coder dispatch moved into
the shared `crates/openagents-chat-app` and `crates/openagents-chat` crates
(`72123d8364`, `81abef60b3`, `43ac875c90`, `6bede2ea46`, `20689a18f3`,
`11cc80897e`, `b8aa0ef52a`, `6dc7ecb717`, `5f5e7dbe9b`). The mobile tests
(142 passed), `openagents-chat` (31), and `openagents-chat-app` (84) pass.
On a fresh iPhone 17 Pro simulator (iOS 26.5, deleted afterwards) the
build-33 app (`3406cb3c49`) was installed first and used for two chats,
then build 34 was installed over it against the live chat worker
(`7ae2a4dd41`): both build-33 chats were in Chats and opened with their
replies, and "Who are you?", already used in build 33, was no longer
suggested; tapping outside the composer dismissed the keyboard; a tapped
suggestion ("What model is this?") answered and did not return on the next
new chat; "Who are you" answered "We are OpenAgents. …" and "How do I
connect a phone" answered "Open OpenAgents for Mac and it shows a QR code.
…" with no `[openagents.` tag; "Explain in three short paragraphs how HTTP
caching works" showed Working, then a partial reply, then the full three
paragraphs; "Run the tests in my repository and fix the failing one"
answered "That needs a computer. …" with a **Connect a computer** chip; and
after the app was quit and relaunched, all five chats were in Chats and
opened with their messages. Build 34 was uploaded with `build.sh upload`
at 2026-09-30T07:02:37-07:00 and is `VALID`, in Internal Testers (`IN_BETA_TESTING`).

TestFlight build 35 (1.0.0, archived from `4e3140ed34`, 2026-09-30)
changes no worker release. Since build 34 the phone lists a paired
computer's own threads in Chats and continues them (`6a33d786d7`), stops a
computer's reply over NIP-HOST `thread.stop` (`9920d77f52`, `c40a5d1e15`),
keeps those threads across a relaunch and queues offline follow-ups until
the computer is reachable (`f88d279d71`, `45cfb2b75f`), shows and stops
Coder runs `openagents chat` started on the computer (`31545a96d9`,
`7536ae0235`), and shows which agent a Run Coder offer will use
(`2210173645`); NIP-DEC (`5710e1311c`) is also in. The mobile tests (147
passed), `openagents-chat-app` (117), and `openagents-chat` (48) pass. On a
fresh iPhone 17 Pro simulator (iOS 26.5, deleted afterwards) the build-34
app (`270688fabe`) was installed first and asked "Who are you?" and "How
do I connect a phone", both answered against the live chat worker; then
build 35 was installed over it: it launched, "Who are you?" was no longer
suggested, the build-34 chat was in Chats and opened with both replies,
the changelog showed 1.0.0 (35), and "Who are you" in a new chat answered
"We are OpenAgents. …". The host-thread, stop, offline, and Coder-run
paths need a paired computer and were covered by the mobile tests, not on
the simulator. Build 35 was uploaded with `build.sh upload` at
2026-09-30T14:51:16-07:00 and is `VALID`, in Internal Testers
(`IN_BETA_TESTING`).

TestFlight build 36 (1.0.0, archived from `1a2ccbb256`, 2026-09-30)
changes no worker release. Since build 35 the phone composer edits
through the shared editor (a whole emoji or accented letter deletes at
once, Undo and Redo in the field's edit menu), saved chat cards open a
native pin/unpin and archive/restore menu, photos attach to the draft,
transcript code blocks take syntax colors, and a refused send keeps the
draft (`dd6ab030a9`, `e9ef828c54`, #10028). The mobile tests (150
passed), `openagents-chat-app` (117), and `openagents-chat` (48) pass. On a
fresh iPhone 17 Pro simulator (iOS 26.5, deleted afterwards) build 36
launched, the changelog showed 1.0.0 (36), "Who are you" answered "We are
OpenAgents. …", and a request for a Rust function was answered by the live
chat worker with a colored `rust` code block. A photo attached through the
`--coder-tap attach:` hook showed above the composer with **Remove**, and
sending with it showed "Hosted chat accepts text only. Remove the images to
send." with the photo kept. `SharedContractsUITests` (composer grapheme
delete and Undo, card menu Pin and Unpin with an attached image, code
colors) passed on the same simulator against the chat fixture. Build 36 was
uploaded with `build.sh upload` at 2026-09-30T15:21:03-07:00 and is
`VALID`, in Internal Testers (`IN_BETA_TESTING`).

Release `110e98d76b` (2026-09-30) puts `chat-router-v4` live: the
`presentation.open` route and, on a desktop turn, the `deck` question over
the decks the desktop app ships, so a desktop chat that asks to open a deck
gets "Opening {deck}." and an `open_presentation` offer, and the phone and
the terminal get "Decks open in the OpenAgents desktop app, so we can't
show one here." ([#10058](https://github.com/OpenAgentsInc/openagents/issues/10058),
[the measurement](../coder/measurements/2026-09-30-presentation-route.md)).
It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/110e98d76b` with `knowledge/` and
`codebase-kb.gz` copied from `7ae2a4dd41` (`knowledge/openagents/` is
unchanged since), checked with `--check` under the chat environment ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; the environment file and unit
did not change, `coder-worker.service` and `/opt/coder-worker/current` were
not touched, and `7ae2a4dd41` stays in `releases/` for rollback. The log
names `router chat-router-v4@12ea6aac036f (Live), bank
chat-answers-v1@f1b498639ec6 with 61 answers` and `calibration off`.
The live check could not reach the router: from 23:41 UTC, before this
release went live, every Jev call returned HTTP 402 (the TypeSafe
organization has no credits), so `live_basic_coder_streams_a_reply` with
`OPENAGENTS_TEST_CHAT_SURFACE=desktop` and "open the Test-Time
Capabilities deck" got a model reply with no judgment. Every turn is the
model's alone until credit is added (`NEEDS_OWNER.md`); the router's
readings on the new rows are in the measurement, from the hosted Jev eval
run before the credits ran out.

Release `5ed35bf130` (2026-10-01 UTC,
[#10064](https://github.com/OpenAgentsInc/openagents/issues/10064)) gives
the router judge Jev's fallback doors (the Vercel AI Gateway, then
OpenRouter) and names the answering door in each judgment. It was built
with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/5ed35bf130` with `knowledge/` and
`codebase-kb.gz` copied from `110e98d76b`, checked with `--check` under the
chat environment ("the configuration is safe to deploy"), and put live by
moving the `chat` symlink; the owner's new OpenRouter key replaced the old
one in the environment file through
`scripts/decision-worker-install-door-keys.sh`, which restarted
`coder-worker-chat` (the same key serves personalization). The unit did not
change, `coder-worker.service` and `/opt/coder-worker/current` were not
touched, and `110e98d76b` stays in `releases/` for rollback. The log names
`router chat-router-v4@12ea6aac036f (Live)`, the same question set as
before, and:

```text
judge   https://api.typesafe.ai (jev-latest): first response and suggestions
judge   fallback door https://ai-gateway.vercel.sh off: $AI_GATEWAY_API_KEY is not set
judge   fallback door https://openrouter.ai under $OPENROUTER_API_KEY
```

`openagents chat --scratch --json "How do I connect a phone"` answered
"Open OpenAgents for Mac and it shows a QR code. …" (route `product.kb` at
0.98) with the judgment naming `door` `https://api.typesafe.ai` and `model`
`jev-1.13.0`: TypeSafe answers again, and no fallback door was asked.

TestFlight build 37 (1.0.0, archived from `b63e21d90f`, 2026-09-30)
changes no worker release. Since build 36 a message with photos sends: its
words go to the hosted router and the photos stay in the draft, bound to
that message; **Run Coder** carries their exact bytes to the paired
computer, and a reply that does not lead to Coder keeps them with "Images go
only to Coder, and this reply didn't start it. They stay in your draft."
(`3e13917001` #10066, `1ebd5e20f7` #10070). A finished Coder run's card
names the exact revisions it compares, marks a moved change stale with
**Refresh**, and publishes once as a draft PR or commit read over
`task.review` / `task.publish` (`ebdec8d004`, #10067, #10068). The mobile
tests (154 passed), `openagents-chat-app` (133), and `openagents-chat` (50)
pass. On a fresh iPhone 17 Pro simulator (iOS 26.5, deleted afterwards)
build 37 launched and the changelog showed 1.0.0 (37). A photo attached
through the `--coder-tap attach:` hook and "What is OpenAgents?" sent with
it: the live chat worker answered, and the photo stayed above the composer
with **Remove** and the "Images go only to Coder…" line. Run Coder with a
screenshot and the review/publish card need a paired computer and were
covered by the mobile and chat-app tests, not on the simulator. Build 37
was uploaded with `build.sh upload` at 2026-09-30T18:42:06-07:00 and is
`VALID`, in Internal Testers (`IN_BETA_TESTING`).

Release `92aef353b7` (2026-10-01 UTC,
[#10073](https://github.com/OpenAgentsInc/openagents/issues/10073)) routes
an explicit request to delegate to Coder, such as "do a test delegation
now", to `work.dispatch` instead of a Gym test: the rubric moves the
question set to `chat-router-v4@ca74e9da5045` with a refit
`calibration-v2.json`
([the measurement](../coder/measurements/2026-09-30-delegation-route.md)).
It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/92aef353b7` with `knowledge/` and
`codebase-kb.gz` copied from `5ed35bf130` (`knowledge/openagents/` is
unchanged since; no `._*` files), checked with `--check` under the chat
environment ("the configuration is safe to deploy"), and put live by moving
the `chat` symlink and restarting `coder-worker-chat`; the environment file
and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `5ed35bf130` stays in
`releases/` for rollback. The log names `router
chat-router-v4@ca74e9da5045 (Live), bank chat-answers-v1@f1b498639ec6 with
61 answers` and `calibration off`. With `openagents chat --scratch
--no-run --json`, the owner's three turns ("who are you", "who can you
delegate to", "do a test delegation now") routed the last to
`work.dispatch` at 1.0 with lane `computer`; "Test Project map on Coder"
still answered with the Project map card and `start_eval`; and "who can you
delegate to" stayed `meta` at 0.99. Use `--no-run` for these checks: without
it, a dispatch reply starts Coder in this checkout.

TestFlight build 38 (1.0.0, archived from `4ceaa4f542`, 2026-09-30)
changes no worker release; it ships the phone side of release
`92aef353b7`. Since build 37 one reply never both offers a Gym test and
starts Coder: an explicit `run_coder` offer offers Coder, else another
typed action is what the router chose, else the computer lane offers it,
and the dispatch's own **Connect a computer** stays part of the Coder offer
(`92aef353b7`, `39b21e3694`, #10073). Run Coder titles and starts the run
with the message that asked for the work, and decision-model calls
(`openagents.decision-call.v1`, such as Microcoder's judge) show no
transcript row. A computer with the update also lifts a usage-limit refusal
a later probe reading contradicts and labels a task worktree by its
repository. The mobile tests (154 passed), `openagents-chat-app` (134), and
`openagents-chat` (54) pass. On a fresh iPhone 17 Pro simulator (iOS 26.5,
deleted afterwards) build 38 launched and the changelog showed 1.0.0 (38).
With `--chat-script "who can you delegate to|do a test delegation now"`
against the live chat worker, the first reply named Coder and the second
answered "That needs a computer. Connect one and we'll dispatch Coder there
with this conversation." with a **Connect a computer** chip, no Gym test
card, and no `openagents.microcoder.judge` row. Run Coder on a paired
computer, the run's title, and the host-side usage and project labels were
covered by the tests, not on the simulator. Build 38 was uploaded with
`build.sh upload` at 2026-09-30T19:41:40-07:00 and is `VALID`, in Internal
Testers (`IN_BETA_TESTING`).

Release `ff3a99aad6` (2026-10-01 UTC,
[#10077](https://github.com/OpenAgentsInc/openagents/issues/10077)) makes a
chat on a computer know it is on that computer: each turn's `context` names
where Coder runs (`computer`) and the chat's project folder (`project`), the
model is told so, and prepared and knowledge answers follow the chat's place
([the router design](../coder/design/2026-09-28-chat-router.md#where-the-chat-runs-10077)).
The route question did not change (`chat-router-v4@ca74e9da5045`, no
recalibration). It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/ff3a99aad6` with the current
`knowledge/openagents/` (two entries tagged `off-computer`; copied with
`COPYFILE_DISABLE=1`, no `._*` files) and `codebase-kb.gz` from
`92aef353b7`, checked with `--check` under the chat environment ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; the environment file and unit
did not change, `coder-worker.service` and `/opt/coder-worker/current` were
not touched, and `92aef353b7` stays in `releases/` for rollback. The log
names `router chat-router-v4@ca74e9da5045 (Live), bank
chat-answers-v1@b0dcde048be6 with 67 answers` and `product kb
openagents-product@4085953fbecf (60 entries)`. Live checks:

- `live_basic_coder_streams_a_reply` with `OPENAGENTS_TEST_CHAT_SURFACE=desktop`
  and `OPENAGENTS_TEST_CHAT_PROJECT=/Users/someone/work/openagents`: "whats
  your working dir" answered "Our working directory is
  `/Users/someone/work/openagents`." (route `cli` at 0.32, so the model
  answered from its note; no offer, no "connect a computer"); "how do I
  connect another computer" answered from product knowledge with the QR,
  nearby, and `openagents connect invite` steps.
- The same test as a phone with no computer: "Run the tests in my
  repository and fix the failing one" answered "That needs a computer.
  Connect one and we'll dispatch Coder there with this conversation." with
  the Computers offer.
- `openagents chat --scratch --no-run --json` from a checkout: "whats your
  working dir" named the checkout's path; "how do I connect another
  computer" gave the pairing steps.

Release `fac6e46861` (2026-10-01 UTC,
[#10076](https://github.com/OpenAgentsInc/openagents/issues/10076)) reads
which coding engine the person names: a typed `engine` question beside the
others, read only for a dispatch, puts NIP-CJ `engine` on the `run_coder`
offer and serves `dispatch.engine_stem`
([the measurement](../coder/measurements/2026-09-30-engine-request.md)).
The route question is unchanged (`chat-router-v4@ca74e9da5045`); the bank
is `chat-answers-v1@d3ff1e68a58a` with 68 answers and `calibration-v2.json`
is refit for it (serving keeps calibration off). It was built with `cargo
zigbuild` from `fac6e46861` (rebased on `7153cd64bf`, which carries
#10077's `ff3a99aad6`), installed as `/opt/coder-worker/releases/fac6e46861`
with `knowledge/` and `codebase-kb.gz` copied from `ff3a99aad6`, checked
with `--check` under the chat environment, and put live by moving the
`chat` symlink and restarting `coder-worker-chat`; `0152ba7866` (the same
change before the engine stem closed with its generic end) and
`ff3a99aad6` stay in `releases/` for rollback, and `coder-worker.service`
and `/opt/coder-worker/current` (`0757355c1d`) were not touched. With
`openagents chat --scratch --no-run --json` from a checkout, "Do a test
delegation to claude" answered "We'll dispatch Coder, asking for Claude
Code, to take this on, with this conversation as its task." with a
`run_coder` offer carrying `engine: claude_code` and the prediction "You
asked for Claude Code; it will do this."; "delegate this" got a
`run_coder` offer with no engine ("Codex will do this."); and "ask Claude
what a monad is" stayed `general` with no offer. `--no-run` now reports
this computer's readiness, so its offer is the one a run would take.

TestFlight build 39 (1.0.0, archived from `846965af1c`, 2026-10-01 UTC)
changes no worker release; it ships the phone side of releases
`ff3a99aad6` (#10077) and `fac6e46861` (#10076). Each phone turn's
`context` names the paired computer by its label (`computer: paired`),
even while it is offline, so the chat does not ask to connect one. In a
computer's chat opened on the phone, Run Coder (`thread.run`) starts on the
engine the reply's `run_coder` offer names, and a Coder run started from a
chat shows the person's message once with "Continued from the OpenAgents
app" as its note. A Run Coder from the phone's own chat creates the task
with the prompt only (`start_task_with_images`), so the computer does not
learn the requested engine there; that start uses the computer's own
order. #10075's chip sizing changes no phone surface: the phone draws its
chips outside the transcript, and its transcript buttons are neither pills
nor `intrinsic_width`. The mobile tests (154 passed), `openagents-chat-app`
(136), and `openagents-chat` (59) pass. On a fresh iPhone 17 Pro simulator
(iOS 26.5, deleted afterwards) build 39 launched and the changelog showed
1.0.0 (39). With `--chat-script "do a test delegation to claude"` and no
computer paired, the live chat worker answered "That needs a computer.
Connect one and we'll dispatch Coder there with this conversation." with a
**Connect a computer** chip drawn as before, no Gym test card, and no
judge row; that reply does not name Claude Code. Engine starts on a paired
computer, the start card's reason, and the handoff note were covered by
the tests, not on the simulator. Build 39 was uploaded with `build.sh
upload` at 2026-09-30T20:57:18-07:00 and is `VALID`, in Internal Testers
(`IN_BETA_TESTING`).

TestFlight build 40 (1.0.0, archived from `d8f7c3db4f`, 2026-10-01 UTC)
changes no worker release. Its source is the code the release acceptance
gate passed 13/13 at `1c095e50d0` plus the changelog entry and build
number; `crates/`, `bins/`, and `apps/` otherwise match `1c095e50d0`. It
ships the phone side of #10081, #10079, and #10084, and #10078 on the
computer. Run Coder from the phone's own chat now names the reply's
requested engine in `task.create` (`engine`) to a host whose presence
advertises `task-engine`, so the note in build 39's record no longer
holds; an older host gets the request it always got. A reply that
answered on the computer lane offers no Coder (#10079), and the shared
delegation prompt tells the engine its delegation is done (#10084).
The mobile tests (155 passed), `openagents-chat-app` (136), and
`openagents-chat` (61) pass. On a fresh iPhone 17 Pro simulator (iOS
26.5, deleted afterwards) build 40 launched and the changelog showed
1.0.0 (40). A plain message got a one-sentence reply, and with
`--chat-script "do a test delegation to claude"` and no computer paired
the live chat worker answered "That needs a computer. Connect one and
we'll dispatch Coder there with this conversation." with a **Connect a
computer** chip, no Gym test card, and no judge row. Engine requests to a
paired computer, the start card's reason, the delegation prompt, and the
wide-checkout start were covered by the tests, not on the simulator.
Build 40 was uploaded with `build.sh upload` at 2026-09-30T22:57:26-07:00
and is `VALID`, in Internal Testers (`IN_BETA_TESTING`).

Release `4bde215a83` (2026-10-01 UTC,
[#10087](https://github.com/OpenAgentsInc/openagents/issues/10087)) says
*plugin*: the answer bank's Gym and missing-plugin entries and the
product knowledge entries the chat serves call anything a person adds a
plugin, and a request no plugin covers gets "There's no plugin for that
yet. Want to make one?". The question set is unchanged
(`chat-router-v4@ca74e9da5045`), so no recalibration was needed; the bank
is `chat-answers-v1@9b39e6796962` with 68 answers, and serving keeps
calibration off. It was built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/4bde215a83` with the current
`knowledge/openagents/` (60 entries, no `._*` files) and `codebase-kb.gz`
copied from `fac6e46861`, checked with `--check` under the chat
environment ("the configuration is safe to deploy"), and put live by
moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` (`0757355c1d`) were not touched, and
`fac6e46861` stays in `releases/` for rollback. With `openagents chat
--scratch --no-run --json`, "What's a plugin?" was answered from the bank
(`gym.what_tool@2`, "A plugin is anything you add to OpenAgents. …"), and
"Can you book me a flight to Tokyo next week?" took `capability.missing`
and answered "There's no plugin for that yet. Want to make one? …"
(`capability.missing@2`).

Release `f04b34bad8` (2026-10-01 UTC,
[#10085](https://github.com/OpenAgentsInc/openagents/issues/10085)) opens
the desktop app's route map from chat: "show me how you route things" stays
on `meta`, whose rubric now covers asking to see how we route or are put
together, and is answered by `meta.map` off the desktop or, when the
request's `context.surface` is `desktop`, by `meta.map.desktop` with NIP-CJ
`open_screen` `routes.map` ("Open the map"). The question set's digest moved
with the rubric (`chat-router-v4@398b034caf30`), the bank is
`chat-answers-v1@8b7c39100770` with 70 answers, `calibration-v2.json` was
refit ([measurement](../coder/measurements/2026-10-01-route-map-route.md)),
and serving keeps calibration off. It was built with `cargo zigbuild` as
above, installed as `/opt/coder-worker/releases/f04b34bad8` with the current
`knowledge/openagents/` (64 entries, no `._*` files; the worker logs 63
admitted) and `codebase-kb.gz` copied from `4bde215a83`, checked with
`--check` under the chat environment ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `4bde215a83` stays in `releases/` for rollback. The log
names `router chat-router-v4@398b034caf30 (Live), bank
chat-answers-v1@8b7c39100770 with 70 answers`. `openagents chat --scratch
--no-run --json "show me how you route things"` answered from the bank
(`meta.map@1`, no offer, as a terminal turn should), and the release gate's
`route-map-chat` scenario, asking as the desktop app, got
`meta.map.desktop@1` with the `routes.map` offer, whose tap opened the Map
page.

Release `4887ff17b2` (2026-10-01 UTC,
[#10090](https://github.com/OpenAgentsInc/openagents/issues/10090)) gives
the chat every plugin in the Gym from the hosted runner's catalog
(`deploy/eval-runner/catalog`): "Which plugins are in the Gym?" is
grounded on the generated `openagents.plugin-list` note, "What plugins can
I test?" gets the bank's `eval.run.choose` naming all six before Project
map's test set, each plugin's note takes its summary from its
`package.json`, and on the desktop "open the map" is the route map, not the
Project map plugin. The route rubric moved the question set to
`chat-router-v4@4438ef518b3d`; the bank is `chat-answers-v1@73f988d8ec5a`
with 71 answers; `calibration-v2.json` was refit from the published run
([measurement](../coder/measurements/2026-10-01-plugin-catalog-route.md)),
and serving keeps calibration off. It was built with `cargo zigbuild` as
above, installed as `/opt/coder-worker/releases/4887ff17b2` with the
current `knowledge/openagents/` (65 files, no `._*` files; the worker logs
64 entries) and `codebase-kb.gz` copied from `f04b34bad8`, checked with
`--check` under the chat environment ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `f04b34bad8` stays in `releases/` for rollback. The log
names `router chat-router-v4@4438ef518b3d (Live)`, `gym records: 6 tools,
3 builds, 8 notes`. From a fresh key with the phone's context
(`live_basic_coder_streams_a_reply`): "Which plugins are in the Gym?"
listed all six with their package lines (grounded on `product.kb`);
"What does the Dependency check plugin do?" was answered from our note
(`openagents.tool-dependency-check@2`); "What plugins can I test?" named
all six and offered Project map's test set with its card. With the
desktop's context, "open the map" and "show me how you route things" got
`meta.map.desktop@1` with **Open the map** (`routes.map`), and "test
project map" got the Project map card and **Start the test**. The release
gate's new `plugins-chat` scenario and `route-map-chat` passed against it.

TestFlight build 41 (1.0.0, archived from `2634579ad3`, 2026-10-01 UTC)
changes no worker release. Its source is the code the release acceptance
gate passed 17/17 at `063044d929` plus the changelog entry, the build
number, and two README words; `crates/`, `bins/`, and `apps/` otherwise
match `063044d929`. It ships the phone side of #10087 (the app says
*plugin*: **Test a plugin**, with and without the plugin, **NO PLUGIN FOR
THAT YET** with **ADD A PLUGIN**), #10086 and #10090 (the Gym's chips and
the computer prompt know all six catalog plugins), and #10089 (the phone
takes `plugin list` as well as `ext list`; the worker still offers `ext
list`). "Chat on a paired phone names your computer" is #10077, which
shipped in build 39, so build 41's entry does not repeat it. The mobile
tests (155 passed), `openagents-chat-app` (154), and `openagents-chat`
(63) pass. On fresh iPhone 17 Pro and iPhone 17 Pro Max simulators (iOS
26.5, deleted afterwards) build 41 launched and the changelog showed
1.0.0 (41). With `--chat-script` and no computer paired, the live chat
worker answered "which plugins can I test?" with `eval.run.choose`
naming Explain this error, Release notes, and Dependency check in its
text, the Project map card (**Start the test**, "6 tests, with and
without the plugin."), and chips for Code finder, Test reader, Explain
this error, Release notes, and Dependency check; "Can you book me a
flight to Tokyo next week?" got "There's no plugin for that yet. Want to
make one?" with the **NO PLUGIN FOR THAT YET** card and **ADD A
PLUGIN**; a new chat showed the **Test a plugin** question chip. The
start of the plugins reply scrolled above the screen and was not read
on the simulator, and `plugin list` on a paired computer was covered by
the tests, not on the simulator. Build 41 was uploaded with `build.sh
upload` at 2026-10-01T01:06:03-07:00 and is `VALID`, in Internal Testers
(`IN_BETA_TESTING`).

Release `9afc94bd92` (2026-10-01 UTC,
[#10094](https://github.com/OpenAgentsInc/openagents/issues/10094)) makes a
follow-up after a Coder run the router's: a turn whose `context.coder_run`
says the chat's run ended gives its summary, files, and commands to the
chat model's instructions and puts only a fixed line that it ended before
the latest message for Jev, so "summarize what happened" is answered in
chat and "now add a test" is a dispatch that continues the same task. The
rubric moved the question set to `chat-router-v4@1d266532e7d0`; the bank is
`chat-answers-v1@43063f287db6` with 71 answers (`meta.privacy@3`);
`calibration-v2.json` was refit from the published run
([measurement](../coder/measurements/2026-10-01-coder-followup-route.md)),
and serving keeps calibration off. It was built with `cargo zigbuild` as
above, installed as `/opt/coder-worker/releases/9afc94bd92` with the
current `knowledge/openagents/` (65 files, no `._*` files; the worker logs
64 entries) and `codebase-kb.gz` copied from `4887ff17b2`, checked with
`--check` under the chat environment ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `4887ff17b2` stays in `releases/` for rollback. The log
names `router chat-router-v4@1d266532e7d0 (Live)`. Against it, the release
gate's `delegate-now`, `followup-chat` ("summarize what happened" answered
on `general` from the run's result, no new Coder turn), `followup-coder`
("now also list the top-level files in a note" continued the same task as
turn 2 and finished, the message above its card), and `delegate-claude`
passed with real engines, debug builds (`--bin-dir`).

TestFlight build 42 (1.0.0, archived from `a4a39f0752`, 2026-10-01 UTC)
changes no worker release. Its source is `origin/main` at `3ce548bd7d` plus
the changelog entry and the build number. It extends the **Text only**
entry (#10093, #10095, no attach button and no way to attach) with the
phone side of #10094 (after a Coder run ends, a question about it is
answered in chat and a request for more work continues the same task; the
worker release `9afc94bd92` above), #10100 (the newest chat is at the top
of Chats, Pinned above, with the project as a row label), and #10091 and
#10092 (Grok Build is allowed by default on a computer). The mobile tests
(157 passed), `openagents-chat-app` (162), and `openagents-chat` (66)
pass. On a fresh iPhone 17 Pro simulator (iOS 26.5, deleted afterwards)
the changelog showed 1.0.0 (42) with the four items, the chat had no
attach button, and the live chat worker answered "Who are you?". The
newest-first order, the follow-up routing with a finished Coder run, and
Grok Build on a paired computer were covered by the tests and not run on
the simulator or a device. Build 42 was uploaded with `build.sh upload` at
2026-10-01T07:37:31-07:00 and is `VALID`; it shows `IN_BETA_TESTING` for
Internal Testers, which receives builds without a manual assignment (the
API refuses to add an internal group by hand).

Release `c59ec00d1a` (2026-10-01 UTC,
[#10099](https://github.com/OpenAgentsInc/openagents/issues/10099)) makes
our two essays, [Test-Time Capabilities](../essays/2026-09-29-test-time-capabilities.md)
and [The Return of the General Agent](../essays/2026-10-01-the-return-of-the-general-agent.md),
part of what the chat answers from: 42 entries in `knowledge/openagents/`
(an overview and an entry per section of each essay), and the route rubric
now has `product.kb` cover what the essays say, so "what's a capability
claim?" and "what's your thesis about general agents?" no longer read
`general`. The set is `chat-router-v4@ef02faf055e7`, the bank is unchanged
(`chat-answers-v1@43063f287db6`), `calibration-v2.json` was refit
([measurement](../coder/measurements/2026-10-01-essays-route.md)), and
serving keeps calibration off. It was built with `cargo zigbuild` as above,
installed as `/opt/coder-worker/releases/c59ec00d1a` with the current
`knowledge/openagents/` (107 files, no `._*` files; the worker logs 106
entries) and `codebase-kb.gz` copied from `9afc94bd92`, checked with
`--check` under the chat environment ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `9afc94bd92` stays in `releases/` for rollback (`2002526d7f`,
the same binary with the knowledge alone, was live for a quarter of an hour
before it). The log names `router chat-router-v4@ef02faf055e7 (Live)` and
`product kb openagents-product@904767b561cf (106 entries)`. From a fresh key
with `openagents chat --scratch --no-run --json`, "what is a test-time
capability?", "what's a capability claim?", "what is a capability delta?"
(each from its entry), "what's your thesis about general agents?", "why do
typed decision models make routing feasible?", "what have you shown and not
shown?", and "why did general agents stall?" all took `product.kb` and
answered from the essays' entries; the release gate's `essays-chat`
scenario passes against it.

Release `54fd3851a0` (2026-10-01 UTC,
[#10102](https://github.com/OpenAgentsInc/openagents/issues/10102)) answers
requests to summarize, explain, or compare our essays from the essays'
overview entries instead of offering Coder, serves the typed `routes.map`
offer for requests to open the route or plugin map on the desktop, and
changes the `explain` opener to "We'll look that up for you." The set is
`chat-router-v4@71bde73d1610`, the bank `chat-answers-v1@e158a330ddca` (71
answers), `calibration-v2.json` was refit
([measurement](../coder/measurements/2026-10-01-essays-route.md#summaries-of-our-essays-and-the-map-on-the-desktop-10102)),
and serving keeps calibration off. The product note now asks the model to
give the link of a document it summarizes (`router::PRODUCT_LINKS`), and the
citation tidier drops a comma that only joined two citations. It is built on
`509575c549` (the `web` surface, #10106), so that release's policy stays. It
went live in three steps the same hour: `4ff0eac4d4` (router, bank, and the
two overview entries at version 2: `knowledge/openagents/` 107 files, no
`._*` files, the worker logs `product kb openagents-product@fd15cb6fdc5d
(106 entries)`), `7cab5e2191`, then `54fd3851a0` (the link rule reaching the
live note). Each was built with `cargo zigbuild` as above, installed under
`/opt/coder-worker/releases/` with `knowledge/` and `codebase-kb.gz` copied
from the release before it (`codebase-kb.gz` from `509575c549`), checked
with `--check` under the chat environment ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; the environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `509575c549`, `4ff0eac4d4`, and `7cab5e2191` stay in
`releases/` for rollback. TypeSafe still answers 402, so every judgment
is answered by the Vercel AI Gateway door (`NEEDS_OWNER.md`).

Live checks from fresh keys, as a phone (`live_basic_coder_streams_a_reply`
with a computer ready), as the desktop (`OPENAGENTS_TEST_CHAT_SURFACE=desktop`
with a project here), and from the terminal (`openagents chat --scratch
--no-run --json`):

- "summarize both of the essays", "summarize your essay on general agents",
  "what are your essays about?", and "compare the two essays" took
  `product.kb`, tier grounded, citing `openagents.ttc-overview` and
  `openagents.gen-overview` (and section entries for the single essay), with
  a summary of each essay and its GitHub link, on every surface.
- "what makes a claim externally validated?" took `product.kb` and served
  `openagents.ttc-term-validated@1` whole.
- "summarize the README in this repo" still took `work.dispatch` (the phone
  and the terminal "We'll have Coder look through the README in your repo";
  the desktop, with no signed-in engine in the check, the no-agent-here
  line).
- On the desktop, "open the route map", "open the plugin map", "show me the
  route map", "show me the map of routes", "show me the map of plugins",
  "how are you put together? show me the whole thing", "draw the
  composition: …", and "can you open the map for me" were served
  `meta.map.desktop@1` with `OpenScreen { screen: RoutesMap }`; "test the
  project map plugin" and "test project map" took `eval.run` with Project
  map's card (`start_eval`). "what's missing in openagents right now, show
  me the gaps" still read `meta` with the model answering and no offer.
- "explain how nostr relays work" opened with "We'll look that up for you."

One phone "compare the two essays" on `7cab5e2191` ran the product lookup
past its budget (each Jev call tries TypeSafe's 402 first) and the model
alone asked for the essays' text; the same question answered from the
overviews on every other try.

Release `0cd4b87152` (2026-10-01 UTC,
[#10110](https://github.com/OpenAgentsInc/openagents/issues/10110)) keeps a
slow Jev judgment from leaving a turn unrouted, and puts the Vercel AI
Gateway first among Jev's doors at the owner's direction (the gateway routes
Jev to TypeSafe itself). The judge asks the gateway, then OpenRouter, then
TypeSafe direct last, and skips a door that answered 401 or 402 for five
minutes. On a turn that asks for a first response the model's words wait for
the judgment up to 6 s, with "We'll look that up for you." shown past 2.5 s,
and an unrouted reply carries the fixed `first::UNROUTED_NOTE`. It contains
#10109's chat model door (`c1f9d8d9d9`, Space Bunny Alpha first). It was
built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/0cd4b87152` with `knowledge/` and
`codebase-kb.gz` copied from `c1f9d8d9d9`, checked with `--check` as root
with the chat environment sourced ("the configuration is safe to deploy"),
and put live by moving the `chat` symlink and restarting
`coder-worker-chat`. The environment file and unit did not change,
`coder-worker.service` and `/opt/coder-worker/current` (`0757355c1d`) were
not touched, and `c1f9d8d9d9` stays in `releases/` for rollback. The log
names:

```text
judge   doors https://ai-gateway.vercel.sh → https://openrouter.ai → https://api.typesafe.ai (jev-latest): first response and suggestions
judge   door https://ai-gateway.vercel.sh under $AI_GATEWAY_API_KEY
judge   door https://openrouter.ai under $OPENROUTER_API_KEY
```

From fresh keys with `openagents chat send --scratch --no-run --json`, every
judgment was answered by the gateway (`judge answered by door
https://ai-gateway.vercel.sh` in 354–899 ms), so TypeSafe's 402 was never
reached. "What's new in the Gym?" took `gym.news` and answered from the
Gym's records (4 cited, 8.2 s: Space Bunny missed its first words in 4 s and
Gemini 2.5 Flash took it); "What is Jev?" was the bank's `meta.jev` in
356 ms; "How do I connect a phone" was the product entry about the QR code
(`kb:product`) on two of three tries, with the model winning the knowledge
race on the third; "What's a plugin?" and "Write a haiku about rain" were
the model's under the judged opener.

Release `7b63f16318` (2026-10-01 UTC,
[#10109](https://github.com/OpenAgentsInc/openagents/issues/10109)) answers
on Space Bunny Alpha (`stealth/space-bunny-alpha`) through OpenRouter at
`reasoning.effort` `low` first, with Gemini 3.8 Flash on the Vercel AI
Gateway taking, per turn, any turn the primary fails before its first
words: an HTTP error (a 400 or 404 for a model OpenRouter no longer
serves, a 429), a failure event, an empty stream, nothing at all in 4 s, or,
while it streams its reasoning, no answer text by 8 s. The primary needed no
environment change: unset, `CODER_WORKER_PRIMARY` is Space Bunny Alpha
whenever `OPENROUTER_API_KEY` is set, and after OpenRouter retires the model
on 2026-10-05 every turn falls back with no deploy. It went live in three
steps, each built with `cargo zigbuild` as above, installed under
`/opt/coder-worker/releases/` with `knowledge/` and `codebase-kb.gz` copied
from the release before it (the knowledge matched `main` except
`openagents.chat-privacy@2`, which `c1f9d8d9d9` installed from this
change), checked with `--check` as root with the chat environment sourced
("the configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; the environment file and unit
did not change, `coder-worker.service` and `/opt/coder-worker/current`
(`0757355c1d`) were not touched, and every earlier release stays in
`releases/` for rollback:

- `c1f9d8d9d9`, the door. Knowledge turns regressed: the call started with
  the turn spoke before the retrieval answered ("Please paste or upload the
  essay") on two of three "summarize your essay" tries.
- `6766685252`, on #10110's `0cd4b87152`: a retrieval or CLI tier holds the
  running model's words until its seam answers.
- `7b63f16318`: a primary that is streaming its reasoning gets until 8 s for
  its first words. Under the worker's instructions "What is the capital of
  Australia and why was it chosen?" had fallen back at 4 s twice and
  answered in 8 to 9 s on Gemini.

The log names `door    live (stealth/space-bunny-alpha at
https://openrouter.ai/api with reasoning low, then google/gemini-3.8-flash
at https://ai-gateway.vercel.sh for any turn it has not started in 4000 ms
or, thinking, answered by 8000 ms)` and `gym news
stealth/space-bunny-alpha first, then google/gemini-2.5-flash with its
reasoning off`, each answer `job … answered in … ms, … chars, by <model>`,
and each fallback `door stealth/space-bunny-alpha missed its first words
after 4000 ms (door_absent: …); google/gemini-3.8-flash takes the turn`.
Timings from fresh keys with `openagents chat --scratch --no-run --json`
built from `main` (time to the first partial, and to the result; the
opener's partial is the judged line, so the model's first words are the
second):

| Turn | Before (`54fd3851a0`, Gemini) | After (`7b63f16318`) |
| --- | --- | --- |
| "Summarize your essay The Return of the General Agent" (grounded) | lead 1.96 s, model 11.3 s, done 16.9 s | lead 1.39 s, model 2.97 s, done 4.7 s |
| "How is OpenAgents chat different from running Coder on my computer?" | lead 1.96 s, model 7.29 s, done 8.5 s | lead 0.89 s, model 1.80 s, done 3.4 s |
| "What is the capital of Australia and why was it chosen?" | 4.2 s, done 6.4 s | 3.36 s, done 5.2 s |
| "Write a haiku about rain" | opener 1.21 s, model 4.57 s, done 4.9 s | opener 0.81 s, model 4.61 s, done 4.8 s |
| "what model are you?" (`meta.model@1`) | 1.1 s, "Google's Gemini 3.8 Flash through the Vercel AI Gateway" | 0.98 s, "Space Bunny Alpha (an anonymous preview model) through OpenRouter" |

On the website's `/ask` (the dev server on `127.0.0.1:4310`), "Summarize
your essay …" went from 11.2 s to 3.85 s and the Australia question was
done in 2.4 s; "do you store my chats?" answered from the bank naming
OpenRouter, Space Bunny Alpha, the Gemini fallback, and that its provider
may keep prompts and replies but not train on them. One website haiku fell
back (`nothing in 4000 ms`, Gemini answered, 8.3 s): OpenRouter sometimes
sends nothing for 4 s. `live_a_retired_primary_falls_back_to_gemini` in
`crates/coder/tests/gateway_stream.rs` (ignored; run with both keys) forces
the fallback with a slug OpenRouter does not serve: OpenRouter answered 400
`stealth/no-such-model is not a valid model ID` in 247 ms, and Gemini 3.8
Flash answered "The capital of France is Paris." with first words at 2.4 s,
named as the writer; the same test with Space Bunny Alpha answered in
0.57 s.

TestFlight build 43 (1.0.0, archived from `9a340cef5b`, 2026-10-01 UTC)
changes no worker release. Its **Coder starts at once** entry covers
#10101 (a coding reply starts Coder at once on a computer that allows it,
no Run Coder tap), #10104 (Coder approves its own steps by default), #10113
(an agent's tool call reads as a short line), and the essay answers
(#10099, #10102). The mobile tests (161 passed) pass. The release gate's
phone scenarios passed with debug builds (`--bin-dir`) of `2599dcb859` plus
the changelog entry: `phone-claude` (the computer's run started on Claude
Code) and `phone-start-at-once` (a coding reply started exactly one Coder
task with no tap, the chat showing the start with Stop). The gate's
`engine-logins` check failed only on the Grok Build login's remaining time,
which neither phone scenario uses. The one commit between that base and the
archive (`0a596e5520`, #10116) is desktop-only. The gate exercises the
phone's shared Rust Coder tab, not the iOS app itself; nothing was run on a
simulator or a device. Build 43 was uploaded with `build.sh upload` at
2026-10-01T12:43:38-07:00 and is `VALID`, `IN_BETA_TESTING` in Internal
Testers (which has access to every build), with test notes set from the
entry's What to test line.

Release `0a88285ec4` (2026-10-01) reads the coding agents a phone's paired
computer names ([#10119](https://github.com/OpenAgentsInc/openagents/issues/10119)):
`context.computer` `{place: "paired", name, engines}`, from the host's
NIP-HOST presence, and the model is told each agent and its state, that
each Coder run uses one of them, and that "which agents are connected / who
can you delegate to" is answered from them. It was built with `cargo
zigbuild` as above, installed as `/opt/coder-worker/releases/0a88285ec4`
with `knowledge/` (107 entries, no `._*` files) and `codebase-kb.gz` copied
from `e4c57638f9` on the VM, checked with `--check` as root with the chat
environment sourced ("the configuration is safe to deploy"), and put live by
moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `e4c57638f9` stays in
`releases/` for rollback. The log names `router chat-router-v4@71bde73d1610
(Live), bank chat-answers-v1@b08b64b48be8 with 71 answers`. With
`live_basic_coder_streams_a_reply`, `OPENAGENTS_TEST_CHAT_PAIRED=macbook-pro-m5`,
and `OPENAGENTS_TEST_CHAT_ENGINES=codex=ready,claude=ready,grok=ready,opencode=not_enabled,devin=not_enabled`,
"what coding agents are connected?", "Who can you delegate to", and "What
can CODER delegate to" each answered on the model tier (route `meta`) in
1.5 to 2.3 s with that list, such as "The coding agents connected to Coder
are: Codex — ready; Claude Code — ready; Grok Build — ready; OpenCode —
installed but not enabled; Devin — installed but not enabled". Before the
release, the same context without engines got "Coder is connected to your
paired computer, macbook-pro-m5. No Coder session is currently running."
The release gate's `phone-agents` passed with debug builds (`--bin-dir`)
of `0a88285ec4`: the host's presence named Codex, Claude Code, and Grok
Build ready and OpenCode and Devin not enabled, and the reply named each
ready one.

Release `dcc80c9096` (2026-10-02 UTC) points the website's `WEB_NOTE` and
the `openagents.get-the-app` entry (version 3) at openagents.com/download,
the download page's new address. It was built with `cargo zigbuild` (on
`coderos-4080`, from the same commit), installed as
`/opt/coder-worker/releases/dcc80c9096` with `knowledge/` from the
repository (107 files, no `._*` files) and `codebase-kb.gz` copied from
`95d447a4b5` on the VM, checked with `--check` as root with the chat
environment sourced ("the configuration is safe to deploy"), and put live
by moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `95d447a4b5` stays in
`releases/` for rollback. On openagents.com, "fix my repo" was answered
"The OpenAgents app can inspect and fix your repository on your computer,
with Coder handling the coding work. Download it at
openagents.com/download."

Release `74875544bd` (2026-10-02 UTC) grounds questions about us that leave
the prepared answers ([#10135](https://github.com/OpenAgentsInc/openagents/issues/10135),
[#10136](https://github.com/OpenAgentsInc/openagents/issues/10136),
[#10137](https://github.com/OpenAgentsInc/openagents/issues/10137)): `meta`
turns no prepared answer fits, and `product.kb` turns below the grounded bar
or in a close call, read the product knowledge base instead of going to the
model alone, and the `openagents.chat-privacy`, `openagents.pricing`, and
`openagents.chat-limits` entries (version 3) name Jev's gateway route and
say there is no plan, account, quota, throttle, or API key to bring. It was
built with `cargo zigbuild` as above, installed as
`/opt/coder-worker/releases/74875544bd` with `knowledge/openagents/` from the
repository (107 files, no `._*` files) and `codebase-kb.gz` copied from
`dcc80c9096`, checked with `--check` as root with the chat environment
sourced ("the configuration is safe to deploy"), and put live by moving the
`chat` symlink from `cf299747f7` and restarting `coder-worker-chat`; the
environment file and unit did not change, and `cf299747f7` stays in
`releases/` for rollback. Scratch `openagents chat send` runs: "is there an
iphone app?" twice served the `openagents.get-the-app` entry (`kb:product`)
both times, and a five-turn limits and privacy thread answered every
follow-up on the `grounded` tier with no plan, cap, account, or key invented.

Release `37bd623fec` (2026-10-02 UTC) answers clear short questions and
drops opener lines from replies
([#10138](https://github.com/OpenAgentsInc/openagents/issues/10138),
[#10139](https://github.com/OpenAgentsInc/openagents/issues/10139),
[#10140](https://github.com/OpenAgentsInc/openagents/issues/10140)): a
clarify reading loses to a sure prepared answer, a clarify on a later turn
is the model told to read the earlier messages first, an opener line is
never written into a reply (the progress line shows only while a slow
judgment is pending), and `openagents.coder-engines` (version 2) names the
`coder.providers` setting. It was built with `cargo zigbuild` as above,
installed as `/opt/coder-worker/releases/37bd623fec` with `knowledge/` and
`codebase-kb.gz` copied from `7b8cfce37c` (106 entries loaded), checked with
`--check` ("the configuration is safe to deploy"), and put live by moving
the `chat` symlink from `7b8cfce37c` (itself live briefly after
`74875544bd`) and restarting `coder-worker-chat`; the environment file and
unit did not change, and `74875544bd` stays in `releases/` for rollback.
"what is this?" on openagents.com and in a scratch chat served `meta.who`
(`meta.who.here` on a computer); "try that again, I stopped it too soon"
after a reply was answered by the model twice; no reply started with "We'll
look that up for you."; and "how do I make coder use claude code instead
of codex?" answered with `openagents settings set coder.providers
claude,codex,grok`.

Release `cdcc111e85` (2026-10-02 UTC) makes a wallet request mean the
built-in wallet and lets a terminal run read-only commands
([#10170](https://github.com/OpenAgentsInc/openagents/issues/10170)): in a
terminal a sure `wallet` route descends the `wallet` commands (with
`wallet info` when the descent picks none) and a read-only proposal is
answered with `cli.run`; elsewhere the model is told `WALLET_NOTE`; the
route question's wallet and clarify rubrics moved the set to
`chat-router-v4@9d17e4d3e2d7` (bank `chat-answers-v1@cfc839ef703b`). It
followed `ef69c18254` the same day, was built with `cargo zigbuild` as
above, installed with `knowledge/` and `codebase-kb.gz` copied from
`37bd623fec`, checked with `--check`, and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; `37bd623fec` stays for
rollback. The live wallet eval (`crates/coder/tests/wallet_eval.rs`,
`live_hosted_chat`) passed 11 of 11 terminal rows.

Release `abdebf0ba6` (2026-10-02 UTC) makes coding agents opt-out
([#10184](https://github.com/OpenAgentsInc/openagents/issues/10184)): a
computer's `not_enabled` agent now means one the person turned off, the
engines note tells the model never to send the user to Coder's settings to
enable a signed-in agent, and `openagents.coder-engines` (version 3) says
every signed-in agent is used with nothing to enable. It was built with
`cargo zigbuild` on `coderos-4080` (from a clean worktree of that commit),
installed as `/opt/coder-worker/releases/abdebf0ba6` with `knowledge/` from
the repository (107 files, no `._*` files) and `codebase-kb.gz` copied from
`315de9957c`, checked with `--check` ("the configuration is safe to
deploy"), and put live by moving the `chat` symlink from `315de9957c` and
restarting `coder-worker-chat`; the environment file and unit did not
change, `coder-worker.service` and `/opt/coder-worker/current` were not
touched, and `315de9957c` stays in `releases/` for rollback. The log names
`router chat-router-v4@9d17e4d3e2d7 (Live)` and the product KB at 106
entries.

Release `155bf09d01` (2026-10-02 UTC) fans one request out to several
Coder runs ([#10183](https://github.com/OpenAgentsInc/openagents/issues/10183)):
the router asks three more typed questions (`fanout`, `read_only`,
`summarize`), a terminal's sure fan-out is answered with
`dispatch.fan_out` ("Starting 3 read-only runs: Codex, Claude Code, Grok
Build.") and a `run_coder` offer carrying the plan, and a request with
`context.runs` gets the model's combined summary with no routing (bank
`chat-answers-v1@179e56482e87`, route set unchanged at
`chat-router-v4@9d17e4d3e2d7`). It was built with `cargo zigbuild` on
`coderos-4080` from a clean worktree of that commit, installed as
`/opt/coder-worker/releases/155bf09d01` with `knowledge/` and
`codebase-kb.gz` copied from `abdebf0ba6` (the knowledge directory did not
change between them), checked with `--check` ("the configuration is safe
to deploy"), and put live by moving the `chat` symlink from `abdebf0ba6`
and restarting `coder-worker-chat`; the environment file and unit did not
change, `coder-worker.service` and `/opt/coder-worker/current` were not
touched, and `abdebf0ba6` stays in `releases/` for rollback.

Release `fa12c8313d` (2026-10-02 UTC) makes "Help me make a plugin …" in a
terminal the plugin-creation flow
([#10177](https://github.com/OpenAgentsInc/openagents/issues/10177)): the
route question's `eval.author` and `work.dispatch` rubrics moved the set to
`chat-router-v4@60fb0f55d6ab`, and on a terminal turn from the computer
Coder runs on, `eval.author` serves the flow's typed steps as the result's
`plugin` field. It was built with `cargo zigbuild` on `coderos-4080` from a
clean worktree of that commit, installed as
`/opt/coder-worker/releases/fa12c8313d` with `knowledge/` and
`codebase-kb.gz` copied from `155bf09d01`, checked with `--check` ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink and restarting `coder-worker-chat` (after `928c40bd5b` the same
day); the environment file and unit did not change, `coder-worker.service`
and `/opt/coder-worker/current` were not touched, and `155bf09d01` stays in
`releases/` for rollback.

Release `f1409c846d` (2026-10-02 UTC) makes every reply that starts work
begin with the verb and name no one doing it
([#10212](https://github.com/OpenAgentsInc/openagents/issues/10212)):
"Picking up issue #10178.", "Looking through your latest commits to tell
you what changed.", "Working on adding dark mode to your settings page.",
"Starting Claude Code on this.", "Exploring the repo with Codex, Claude
Code, Grok Build, and Devin.", "Running the openagents command for that on
this computer." (it said "We'll have Coder ...", "We'll dispatch ...",
"We're running ..."), and refuses a continuation that opens with a broken
word ([#10178](https://github.com/OpenAgentsInc/openagents/issues/10178)).
It followed `fa12c8313d` (which already carried the #10212 stems), was
built with `cargo zigbuild` on a Boat sandbox, installed with `knowledge/`
and `codebase-kb.gz` copied from `fa12c8313d`, checked with `--check`, and
put live by moving the `chat` symlink and restarting `coder-worker-chat`;
the environment file and unit did not change, `coder-worker.service` and
`current` were not touched, and `fa12c8313d` stays in `releases/` for
rollback. The live hosted rows of `crates/coder/tests/dispatch_ack_eval.rs`
passed: every work reply started with an -ing verb and none said "We'll
have", "We'll dispatch", "Coder will", or "have Coder".

Release `e1a1e394e2` (2026-10-02 UTC) runs a chat on the caller's own
provider keys ([#10176](https://github.com/OpenAgentsInc/openagents/issues/10176)):
a job that names `payer.keys` in `requires` carries the caller's OpenRouter,
Vercel AI Gateway, or TypeSafe keys sealed to the worker, and runs its model,
personalizer, and Jev only on them, or is refused (NIP-CJ "Caller-paid model
calls"); a body naming any other feature is refused `unsupported_feature`; and
the usage log adds `payer`, `payer_provider`, and `payer_fingerprint`
(`coder-worker usage --by payer`). It was built with `cargo zigbuild` on a
Boat sandbox from that commit, installed as
`/opt/coder-worker/releases/e1a1e394e2` with `knowledge/` from the repository
and `codebase-kb.gz` copied from `40bfa2b842`, checked with `--check`, and
put live by moving the `chat` symlink and restarting `coder-worker-chat`; the
environment file and unit did not change, and `coder-worker.service` and
`/opt/coder-worker/current` were not touched. The log names the product KB at
`openagents-product@e94ce986f225` (106 entries). A live turn from a throwaway
key answered on our doors as before, and a turn with a fake OpenRouter key was
refused with "Your OpenRouter key was refused. Update it in Settings.", its
usage line naming `payer: theirs` and a fingerprint and no key. An earlier
try, `40bfa2b842`, shipped knowledge entries the corpus rules refused, which
turned the product KB off; it was rolled back to `f1409c846d` within a
minute, and both stay in `releases/` for rollback (move the symlink back and
restart).

Release `315de9957c` (2026-10-02 UTC) answers a wallet request in plain
words ([#10202](https://github.com/OpenAgentsInc/openagents/issues/10202)):
the wallet overview a terminal runs when the descent picks no command is
`wallet balance` (one sentence), and `WALLET_NOTE` names `wallet balance` and
`wallet address` and tells the model never to mention nodes, networks,
servers, channels, or liquidity; the x402 Lightning node's commands moved to
`openagents x402 node`. It was built with `cargo zigbuild`, installed with
`knowledge/` and `codebase-kb.gz` copied from `cdcc111e85`, checked with
`--check`, and put live by moving the `chat` symlink; the live wallet eval
(`live_hosted_chat`) passed 11 of 11 terminal rows, each balance row with
`wallet balance` and the address row with `wallet address`.

Release `d4310fc4fa` (2026-10-02 UTC) carries the Spark wallet on computers
(#10202): the command tree's `wallet` group is the person's Spark wallet
(`balance`, `address`, `receive`, `send`, `history`, `link`, `restore`), and
the product entries `openagents.wallet` (v2), `openagents.wallet-trust` (v2),
and `openagents.cli` (v3) say the wallet is the same on linked computers. The
binary was built on a Boat sandbox (`cargo build --release --target
x86_64-unknown-linux-musl`, static), installed with `knowledge/` from the
repository at that commit and `codebase-kb.gz` from `e1a1e394e2`, checked
with `--check`, and put live by moving the `chat` symlink from `e1a1e394e2`,
which stays for rollback. A `knowledge/` archive made with macOS `tar`
carries `._*` AppleDouble files that the corpus refuses (the first start
logged `product kb off`); they were deleted and the worker restarted, and it
logged `product kb openagents-product@be62fe0e41ba (106 entries)`. Make such
an archive with `COPYFILE_DISABLE=1 tar`. Scratch chats then proposed
`wallet balance` for "check my wallet balance" and "my sats", `wallet
address` for "whats my wallet address", and no command for a send.

Release `bf30328c27` (2026-10-02 UTC) keeps a chat on the caller's own keys
grounded ([#10176](https://github.com/OpenAgentsInc/openagents/issues/10176)):
a `payer.keys` job's product, codebase, Gym, CLI, and authoring seams are
lent the caller's embedder and Jev on their keys over our shared records,
where they had been off, and its model and privacy answers name the caller's
own key as the door; `openagents.chat-privacy` is v5. The static binary was
built on a Boat sandbox from a clean worktree at that commit (`cargo build
--locked --release --target x86_64-unknown-linux-musl` with `musl-tools`,
sha256 `7b86cf88a121…`), fetched through the sandbox files API, installed with
`knowledge/` from `git archive bf30328c27 knowledge/` (no `._*` files) and
`codebase-kb.gz` from `d4310fc4fa`, checked with `--check`, and put live by
moving the `chat` symlink from `d4310fc4fa`, which stays for rollback; the
environment file and unit did not change, and `coder-worker.service` and
`current` were not touched. The log names `product kb
openagents-product@6259d1d4d956 (106 entries)` and every seam on. Scratch
chats answered "How do I connect a phone" from `openagents.connect-computer@2`
and "What happens to my messages?" from `openagents.chat-privacy@5`, and a
turn with a fake OpenRouter key was refused with "Your OpenRouter key was
refused. Update it in Settings."

Release `16063d5024` (2026-10-02 UTC) adds the `standing.rule` route
([#10157](https://github.com/OpenAgentsInc/openagents/issues/10157)):
something to keep happening on the user's computer, or a change to such a
background rule, is served `standing.rule` in a terminal (whose computer
then compiles the rule and offers to save it) and `standing.elsewhere` on
other surfaces; the set is `chat-router-v5@226865d8437b` (bank
`chat-answers-v1@0a0f10e9df69`). With CoderOS and the Mac out of disk, it was
built with `cargo build --locked --release --target
x86_64-unknown-linux-musl` (`musl-tools`) on a Boat sandbox from a clean
clone at that commit (a static-pie binary, sha256 `61159a04c224…`),
downloaded through the sandbox's artifacts API, installed as
`/opt/coder-worker/releases/16063d5024` with `knowledge/` and
`codebase-kb.gz` copied from `bf30328c27`, checked with `--check` ("the
configuration is safe to deploy"), and put live by moving the `chat`
symlink from `bf30328c27` and restarting `coder-worker-chat`; the
environment file and unit did not change, `coder-worker.service` and
`/opt/coder-worker/current` were not touched, and `bf30328c27` stays in
`releases/` for rollback. The log names `router chat-router-v5@226865d8437b
(Live)` and the product KB at 106 entries. Scratch `openagents chat send`
runs routed "keep my disk above 50 GB free", "only keep 2 agent target
dirs", "tell me whenever a coder run fails", "every morning pull main in
~/work/openagents", and "pause disk cleanup until tomorrow" to
`standing.rule`, and "clean up my disk right now" to `work.dispatch`
([the measurement](../coder/measurements/2026-10-02-standing-rule-route.md#live-end-to-end)).

Release `bbed5d89af` (2026-10-03 UTC, shakeout batch B, 899ee13fe3): the
"no coding agent" first line ("No coding agent is signed in on this
computer yet, so Coder can't run here. …") and the "what can you do"
answer that names the wallet and plugins. Built on this Mac with `cargo
zigbuild --locked --release -p coder --bin coder-worker --target
x86_64-unknown-linux-musl` from a clean worktree at that commit, installed
as `/opt/coder-worker/releases/bbed5d89af` with `knowledge/` from `git
archive bbed5d89af knowledge/` (`._*` files removed) and `codebase-kb.gz`
copied from `16063d5024`, checked with `--check`, and put live by moving
the `chat` symlink from `16063d5024` (kept for rollback) and restarting
`coder-worker-chat`. The environment file and unit did not change, and
`coder-worker.service` and `/opt/coder-worker/current` were not touched.
The log names `router chat-router-v5@226865d8437b (Live)`, bank
`chat-answers-v1@a4b2858f9f09` (75 answers) and the product KB at 106
entries. Fresh-home `openagents chat send` runs returned the new wording.

Release `42fe20c01b` (2026-10-09 UTC, web chat goldens): the website's
`.website` answers, account questions read the product notes, `WEB_NOTE`
names Coder and the download page, and the notes on sign-in, the Claude
key, environments, Coder's sign-in and sync, and deleting chats. The
release before it (`bbed5d89af`, 2026-10-03) was six days behind main: its
bank still said "dispatch" and "a computer you've connected", and its notes
offered a Mac `.dmg`. Built on this Mac with `cargo zigbuild --locked
--release -p coder --bin coder-worker --target x86_64-unknown-linux-musl`
at that commit, installed as `/opt/coder-worker/releases/42fe20c01b` with
`knowledge/` from `git archive 42fe20c01b knowledge/` and `codebase-kb.gz`
copied from `bbed5d89af` (kept for rollback), checked with `--check`, and
put live by moving the `chat` symlink and restarting `coder-worker-chat`.
The environment file and unit did not change. The log names bank
`chat-answers-v1@aa9fa65069b4` (83 answers) and the product KB at 112
entries. The web chat goldens through a local site against it: 67 of 97
(from 25 of 97 on `bbed5d89af`); see `docs/web/chat-goldens.md`.

Release `17ff0e03cc` (2026-10-09 UTC) puts #11106's website-chat fixes
(`33e4e0930f`) and the chat-privacy note v13 live on the failover chain
(`ddab7b9a6e`): the note no longer says chats go to Space Bunny Alpha
(retired by OpenRouter) and names Gemini 3.8 Flash on OpenRouter, then GLM
5.3 Flash and the Vercel AI Gateway. The binary was built on this Mac with
`cargo zigbuild --locked --release -p coder --bin coder-worker --target
x86_64-unknown-linux-musl` at `2e5f61a282` (no crate changes since; sha256
`a2d1bb4c196b…`), installed with `knowledge/` from `git archive 17ff0e03cc
knowledge/` (no `._*` files) and `codebase-kb.gz` copied from `ddab7b9a6e`,
checked with `--check`, and put live by moving the `chat` symlink and
restarting `coder-worker-chat`. The environment file, unit, and
`coder-worker.service`/`current` did not change; the notes held aside in
`/var/lib/coder-worker-chat/kb-held/` were not restored (the release's
`openagents.jev.md` v3 replaces them), and `2e5f61a282` and `ddab7b9a6e`
stay in `releases/` for rollback. The log names `product kb
openagents-product@d9e9f1c97b64 (108 entries)`, `router
chat-router-v5@bcab1427d09d (Live)`, bank `chat-answers-v1@efe69f37d240`
(86 answers), and Jev answering through OpenRouter (the Vercel AI Gateway
still answers 402 and is skipped). From CoderOS, `openagents chat` answered
"What is OpenAgents?", "how do I connect my codebase?", and "what models do
you use?" from the bank, "can I opt out of training?" from the v13 note,
and "what products do you have?" on Gemini, none naming Jev or TypeSafe as
ours.

Release `80438d7296` (2026-10-09 UTC) answers "what products do you have?"
with the reviewed `meta.products` answers (phone, `.here`, `.website`) and
the `openagents.products` note v1, where the model had said we had no
documented list. Built on this Mac with `cargo zigbuild --locked --release
-p coder --bin coder-worker --target x86_64-unknown-linux-musl` at that
commit (sha256 `51631f4da6605d69…`), installed with `knowledge/` from `git
archive 80438d7296 knowledge/` (no `._*` files) and `codebase-kb.gz` copied
from `17ff0e03cc`, checked with `--check`, and put live by moving the `chat`
symlink and restarting `coder-worker-chat`. The environment file, unit, and
`coder-worker.service`/`current` did not change; `17ff0e03cc` stays in
`releases/` for rollback. The log names `product kb
openagents-product@eead65159fd3 (109 entries)`, `router
chat-router-v5@bcab1427d09d (Live)`, and bank `chat-answers-v1@f9883275c95b`
(89 answers). From CoderOS, `openagents chat` answered "what products do you
have?" with the `openagents.products` note, "what do you make" with
`meta.products.here`, and "What is OpenAgents?" with `meta.who.here`.

Release `15020bfbbc` (2026-10-10 UTC) sends privacy questions to the privacy
answers again and answers "what is memory in coder?" (#11106, #11176):
`meta.memory` v3 no longer claims "whether we learn from their chats" (which
pulled "do you train on my chats" and "do you store my chats" to it), says
what Coder's memory is, and answers the `meta`, `product.kb`, and `general`
routes; the new `openagents.coder-memory` note v1 covers the same, since Jev
reads that question at a low route probability (0.35–0.39) and the model
had said we had no documented answer. Built with `cargo zigbuild --locked
--release -p coder --bin coder-worker --target x86_64-unknown-linux-musl` at
that commit on CoderOS (this Mac was short of disk; sha256
`41ad026d02029d35…`), installed with `knowledge/` from `git archive
15020bfbbc knowledge/` and `codebase-kb.gz` copied from `80438d7296`,
checked with `--check`, and put live by moving the `chat` symlink and
restarting `coder-worker-chat`. The environment file, unit, and
`coder-worker.service`/`current` did not change; `80438d7296` stays in
`releases/` for rollback (`23563881d7` and `4d1e95a585`, the two
intermediate releases this fix went through, are there too). The log names
`product kb openagents-product@fbc68010abaf (110 entries)`, `router
chat-router-v5@bcab1427d09d (Live)`, and bank `chat-answers-v1@c81c3764a2f3`
(90 answers). From CoderOS, `openagents chat` answered "do you train on my
chats?" with the `openagents.chat-privacy` note, "how long do you keep my
chats?" from the notes on the retention details, and "what is memory in
coder?" with the `openagents.coder-memory` note shown whole.

Release `8e2b053dfa` (2026-10-10 UTC) gives prepared answers inline
components (#11187, #11113 phase 1): `meta.codebase` v2,
`meta.coder.website` v3, `meta.github.website` v3, and the notes on
connecting a codebase, installing Coder, Coder's sign-in and sync,
connecting a computer, connecting a repository, getting the apps, and
signing in carry a one-line lead and a fenced `openui-lang` block (buttons
that start each flow, copyable install commands per system, numbered
steps). The website draws the block as components; phone and desktop builds
from this commit draw its Markdown fallback; earlier app builds show the
block as a code block. Built with `cargo zigbuild --locked --release -p
coder --bin coder-worker --target x86_64-unknown-linux-musl` at that commit
on this Mac (sha256 `64205c8c8c15dbe5…`), installed with `knowledge/` from
`git archive 8e2b053dfa knowledge/` and `codebase-kb.gz` copied from
`15020bfbbc`, checked with `--check`, and put live by moving the `chat`
symlink and restarting `coder-worker-chat`; `15020bfbbc` stays for
rollback. The log names `product kb openagents-product@d7bef5c538d6 (110
entries)` and bank `chat-answers-v1@5f318e29b725` (90 answers). From
CoderOS, `openagents chat` answered "How do I connect my codebase?" with
`meta.codebase@2` and its block; on openagents.com (web revision
`coder-web-e4d9859dce-20261010023417`) the same question shows the two
cards, "Log in to connect" signed out, the install command tabs, and the
steps.

Release `a32a919471` (2026-10-10 UTC) answers "What is the Verse?" with
`meta.verse@1` on the product.kb and general routes too, with its typed
`open_screen` offer (`screen: "verse"`, **Enter the Grid**), and
`openagents.verse-grid` v3 describes the plain Grid (#11184). Built as
above on this Mac, installed with `knowledge/` from `git archive a32a919471
knowledge/` and `codebase-kb.gz` copied from `8e2b053dfa`, checked with
`--check`, and put live by moving the `chat` symlink and restarting
`coder-worker-chat`; `8e2b053dfa` stays for rollback. A first restart with
the binary alone failed for about two minutes (no `codebase-kb.gz` beside
it) and was rolled back to `8e2b053dfa` before the full release went live:
install `knowledge/` and `codebase-kb.gz` before moving the symlink. The log
names `product kb openagents-product@59328c6dbfc3 (110 entries)` and bank
`chat-answers-v1@00c75b33206a`. From this Mac, `openagents chat --scratch`
answered "What is the Verse?" from `meta.verse@1` with the `verse` offer,
and "Write a haiku about rain" from `google/gemini-3.8-flash`.

Release `156b301714` (2026-10-10 UTC, #11219) puts Gemini on Vertex AI first
for chat and Vertex embeddings for the product notes and Gym records, with
OpenRouter and the Vercel AI Gateway behind it (OpenRouter's account was at
−$2.52 and the gateway answered 402 on every call, so embeddings failed
every 45 s and Gemini came only through OpenRouter). Nothing on GCP had to
change: `aiplatform.googleapis.com` was already enabled on
`openagentsgemini`, and the VM's default service account already had the
`cloud-platform` scope and Vertex access (a metadata-token call from the VM
answered 200). Built on this Mac with `cargo zigbuild --locked --release -p
coder --bin coder-worker --target x86_64-unknown-linux-musl` at that commit
(sha256 `3c19e612f399622d…`), installed with `knowledge/` from `git archive
156b301714 knowledge/` and `codebase-kb.gz` copied from `a32a919471`. The
environment file was backed up as
`/etc/coder-worker/coder-worker-chat.env.bak-156b301714` and gained
`VERTEX_PROJECT=openagentsgemini`, `VERTEX_LOCATION=global`,
`GCE_METADATA_HOST=metadata.google.internal`,
`KB_VERTEX_PROJECT=openagentsgemini`, `KB_VERTEX_MODEL=gemini-embedding-001`,
and `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=vertex`. Checked with `--check`
against the new release's files, then put live by moving the `chat`
symlink and restarting `coder-worker-chat`; `a32a919471` stays for
rollback. The door line names `google/gemini-3.8-flash at
https://aiplatform.googleapis.com → google/gemini-3.8-flash at
https://openrouter.ai/api → …`, and the product KB line `embeddings through
Google Vertex AI`. From this Mac, `openagents chat --scratch` answered
"What is OpenAgents?" from the bank (`meta.who.here@4`, 0.24–0.53 s at the
worker), "Which model are you, and what is a transformer in one sentence?"
and "Explain the difference between a mixture-of-experts model and a dense
model in two sentences." on Gemini, which the journal logs as `by
google/gemini-3.8-flash at aiplatform.googleapis.com` in 1.1–2.3 s per turn
at the worker (1.7–4.2 s end to end through the relay). The codebase index
was still the OpenAI-built one, so its warm-up embedding kept failing on
the 402 until the Vertex-built index shipped (below).

To roll back: `sudo ln -sfn /opt/coder-worker/releases/a32a919471
/opt/coder-worker/chat && sudo cp -p
/etc/coder-worker/coder-worker-chat.env.bak-156b301714
/etc/coder-worker/coder-worker-chat.env && sudo systemctl restart
coder-worker-chat`. To turn only the Vertex chat door off, add
`CODER_WORKER_VERTEX=off` to the environment file and restart.
