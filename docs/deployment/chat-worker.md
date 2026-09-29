# The chat worker: OpenAgents chat in the OpenAgents app

A new OpenAgents app user can chat with OpenAgents before connecting a
computer; work for a computer is dispatched to Coder there. That chat,
*OpenAgents chat*, is served by one `coder-worker` running in
quota mode on the gateway door's Gemini Flash lane. This page is the serving
decision, its limits, and the runbook.

## The serving path

```text
phone (device key) --NIP-CJ 25900, NIP-44--> relay.openagents.com --> chat worker
phone <--27000 partials, 26900 result------- relay.openagents.com <-- chat worker --> AI gateway
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
- **Model.** `google/gemini-3.8-flash` through the Vercel AI Gateway, the
  lane the Coder terminal's chat used (`crates/coder/src/generate.rs`). The
  gateway key is in the worker's environment file on its host and nowhere
  else. No model API key ships in the app.
- **First response and the chat router.** The worker acknowledges every
  admitted turn with `status: processing` at once. With `TYPESAFE_API_KEY`
  set, a turn that asks gets one Jev (System One) judgment run beside the
  model call, the `chat-router-v2` question set (`crates/coder/src/router/`):
  route, prepared answer, whether the reply needs the user's specifics,
  lane, opener, risk, and a command group when a command tree is wired.
  Code decides what is shown. A turn that sends `"router":
  "chat-router-v1"` (build 20 of the app) gets every tier: a whole answer
  from the reviewed bank (`crates/coder/answers/chat-answers-v1.toml`) with
  followup chips, a refusal, a "We'll dispatch Coder to …" stem with a Run
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
- **Personalization (T1).** The chat router's stems ("We'll dispatch Coder
  to …") are finished by a cheap model through `coder::router::personalize`;
  the worker reads `personalize::seam_from_env()` at start, and its
  `router` line names whether personalization is on. It is off unless the
  worker's environment sets it:
  - `CODER_PERSONALIZE=openrouter` turns it on with OpenRouter's
    `google/gemini-2.5-flash-lite`, the measured choice
    ([the chat router design](../coder/design/2026-09-28-chat-router.md#implemented-and-measured-2026-09-28));
    `openrouter:<model>` names another model. `gateway` (or
    `gateway:<lane>`) uses the door key and URL the worker already has and
    the `glm` lane, about twice as slow. `off` or unset turns it off.
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
  card as soon as the records are judged, and runs on Gemini 2.5 Flash
  with its reasoning off through the same gateway and key (#9950);
  `CODER_GYM_NEWS_MODEL` names another model, and `off` keeps news on the
  chat model. The log says `gym news google/gemini-2.5-flash with its
  reasoning off`. A request may name
  `chat-router-v1` (build 20) or `chat-router-v2`; both are routed with
  v2. The authoring interview (`eval.author`, `coder::eval_author`) runs
  on the worker's door and judge; without them it answers with the
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

## Limits

An allowlisted worker answers only keys an operator named. The chat worker
has to answer a key nobody has seen, so it is open under a quota instead
(`coder::relay::quota`, `CODER_WORKER_QUOTA`):

| Limit | Deployed value | Refusal |
| --- | --- | --- |
| Jobs per caller key in any 60 seconds | 6 | `rate_limited`, with `retry_after_ms` |
| Jobs per caller key per UTC day | 40 | `quota_exhausted`, with `retry_after_ms` to midnight UTC |
| Jobs for every caller together per UTC day | 3,000 | `quota_exhausted`, with `retry_after_ms` to midnight UTC |
| Request ciphertext | 96 KiB | `limit_exceeded` |
| Jobs at once | 8 | `busy` |

- The total is the spend bound. A caller can mint any number of Nostr keys,
  so the per-key limits keep one person from using the day, and the total
  caps the day's cost however many keys arrive.
- A metered caller gets conversation jobs only: a delegation is refused
  `not_admitted`, and execution requests are ignored. Keys on
  `CODER_WORKER_ALLOW` are not metered.
- The day's counts are written to `CODER_WORKER_QUOTA_FILE` after every
  admission, so a restart does not start a second day. The per-minute window
  is kept in memory; a restart resets at most one minute. A file that exists
  but does not read stops the worker.
- A job counts when it is admitted, whether or not the door answers.
- The phone sends at most the newest 48 KiB of the conversation and shows
  each refusal from its code: "You're sending messages quickly. Try again in
  40 seconds." or "We've answered all the messages we can for you today…".

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
the judge line must name `https://api.typesafe.ai`, and the admits line must
name the quota. The worker secret is kept with the
owner's secrets as `coder-chat-worker.env`; the gateway key is the owner's
AI Gateway key.

To check the deployed path from a checkout, send one turn from a fresh key,
as a new install does:

```sh
cargo test --manifest-path crates/openagents-mobile/Cargo.toml \
  live_basic_coder_streams_a_reply -- --ignored --nocapture
```

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
3. Run `live_basic_coder_streams_a_reply` (above) from a checkout. If it
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
