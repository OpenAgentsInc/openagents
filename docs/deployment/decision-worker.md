# The decision worker: Jev with no key on the computer

> **Since 2026-10-10 (#11225) this worker is no longer on the default
> path.** A caller with a TypeSafe key asks Jev directly; a caller without
> one asks our own API, `POST https://openagents.com/api/v1/systemone`,
> which asks Jev under its house key, then connected Pylons over Nostr, then
> Gemini on Vertex ([NIP-DEC](../../nips/openagents/NIP-DEC.md), "The
> OpenAgents decision API"). This worker answers only callers that choose
> `OPENAGENTS_DECISIONS=legacy` or a `relay` decision profile.

Coder asks Jev, TypeSafe's System One model, for its judgments. A computer
with a TypeSafe key asks TypeSafe directly. Every other computer asks the
*hosted decision service*: one `decision-worker` that holds the TypeSafe key
on the server and answers NIP-CJ decision jobs on `wss://relay.openagents.com`.
This page is the serving path, its bounds, its usage log, and the runbook.

## The serving path

```text
Coder (decision key) --NIP-CJ 25910, NIP-44--> relay.openagents.com --> decision worker
Coder <--27010 status, 26910 result----------- relay.openagents.com <-- decision worker --> ai-gateway.vercel.sh
                                                                              (then openrouter.ai, then api.typesafe.ai)
```

- **Wire.** Each judgment is one `POST /v1/systemone` call carried as a
  decision job ([NIP-DEC](../../nips/openagents/NIP-DEC.md), the NIP-CJ
  decision family; `crates/nostr/src/decision.rs`): a kind `25910`
  request signed by the computer's decision key and encrypted with NIP-44 to
  the worker, then the worker's `27010` statuses and its `26910` result. The
  relay sees ciphertext and routing tags only, and every kind is ephemeral.
- **Endpoint.** The worker's public key is compiled into clients
  (`jev_hosted::WORKER` in `crates/jev-hosted`):
  `ad6b4d9199bf0864b1a402116d44e36daa1a72c6f8df4ae07bd57c8df5c922fc`, on
  `wss://relay.openagents.com`.
- **Identity.** The computer's decision key, `~/.openagents/decision.key`,
  made on first use (32 random bytes as hex, mode 0600). It signs the job and
  answers the relay's NIP-42 challenge. There is no account and no key to
  paste.
- **The doors.** The worker forwards each admitted job, with
  `Idempotency-Key` and `X-Attempt`, to the Vercel AI Gateway first
  (`typesafe-ai/jev`, under `AI_GATEWAY_API_KEY`), which routes Jev to
  TypeSafe itself; then OpenRouter; then `https://api.typesafe.ai/v1/systemone`
  last, under `TYPESAFE_API_KEY` (the order below). Every key is in the
  worker's environment file on its host. No door key ships to a user's
  computer.
- **Evidence.** Every answer the worker relays carries
  `service: {door: "<the door that answered>", version: "decision-worker@<release>"}`
  and a sealed execution receipt bound to the request's digest. The client
  checks the worker's signature, the request's event id, its own key, and
  the receipt before it believes an answer.

## The doors, in order

The open lane asks Jev's three doors in the chat judge's order (NIP-DEC,
"Doors and the backup door"; `jev::doors`), set by `open.upstream_last`
in the config. The owner chose it on 2026-10-01: the gateway is the
primary and handles provider routing, falling back to TypeSafe with the
owner's own key, and TypeSafe direct is the final backup.

1. The **Vercel AI Gateway**'s TypeSafe-compatible API,
   `POST https://ai-gateway.vercel.sh/typesafe/v1/systemone`, with the same
   `state` and `questions` and the model as `typesafe-ai/jev` (the gateway's
   one Jev, unversioned), under `AI_GATEWAY_API_KEY`.
2. **OpenRouter**'s Decisions API,
   `POST https://openrouter.ai/api/alpha/decisions`, the model as
   `typesafe/jev-1.13`, under `OPENROUTER_API_KEY`.
3. **TypeSafe** direct, the `upstream`, under `TYPESAFE_API_KEY`.

The first two are the `backups` list of the config, and **each is off
unless its key is in the worker's environment file** when it starts; with
both off, TypeSafe answers alone. The journal names each at start
(`backup door … under $AI_GATEWAY_API_KEY`, or `backup door … off:
$AI_GATEWAY_API_KEY is not set`, and the same for OpenRouter) and the
open lane's order (`open lane doors https://ai-gateway.vercel.sh →
https://openrouter.ai → https://api.typesafe.ai`). A provisioned
principal's jobs, under their own key, ask the upstream first and the
backups after it.

- A door is asked only after the doors before it timed out, could not be
  reached, answered 402, 408, 429, or any 5xx, or refused for their own
  reasons (their key, account, model list, rate, or quota;
  `jev::doors::fails_over`). A request the caller got wrong
  (`invalid_request`, …) is never retried at another door, and a door
  after the first that refuses the question ends the chain.
- A door that refused for its key or account (401, 402;
  `jev::doors::benches`) is benched for `jev::doors::BENCH` (five
  minutes): the next jobs skip it as if it refused again, and it is asked
  after the bench ends. The journal says so once (`door … refused for its
  key or account (HTTP 402); skipping it for 300 s`). `bench_secs` in the
  config changes the length; zero turns it off.
- The journal has one line per job naming the door that answered
  (`door https://ai-gateway.vercel.sh answered in 412 ms`), with the first
  door's refusal when another door answered (`…, after
  https://ai-gateway.vercel.sh refused (internal_server_error)`), and a
  line for each door that could not answer.
- Each answer names its door: `service: {door: "https://ai-gateway.vercel.sh", …}`,
  `{door: "https://openrouter.ai", …}`, or `{door: "https://api.typesafe.ai", …}`,
  the model it served (`typesafe-ai/jev`, `typesafe/jev-1.13-20260917`,
  `jev-1.13.0`), and `usage.cost` where the door prices it (the gateway's
  `provider_metadata.gateway.cost`, OpenRouter's own). A client's
  decision record (`openagents.decision-call.v1`) keeps that `service`. If
  no door answers, the caller gets the first door's refusal.
- Every answer, from any door, is one line in the usage log with the door
  that answered and its cost when the door priced it.

The chat worker's router judge asks the same doors, in the same order,
with its own keys ([chat worker](chat-worker.md)).

### Turning the backup doors on

Put the keys in the worker's environment file and restart it. From a
development Mac, with the keys in `~/work/.secrets/ai-gateway.env`
(`AI_GATEWAY_API_KEY=…`) and `~/work/.secrets/openrouter.env`
(`OPENROUTER_API_KEY=…`), an agent runs:

```sh
scripts/decision-worker-install-door-keys.sh
```

It copies only those two variables (never printing them) into
`/etc/decision-worker/decision-worker.env` and
`/etc/coder-worker/coder-worker-chat.env` on `oa-coder-worker-1` through
IAP, keeps every other line, restarts `decision-worker` and
`coder-worker-chat`, and prints the startup lines that name each door on
or off. It never touches `coder-worker.service` or `/opt/coder-worker/current`.

## One resolver for every caller

`jev_hosted::resolve` is how every Jev caller finds Jev:

1. A local TypeSafe key, unchanged: `TYPESAFE_API_KEY`, else `api_key` in
   `~/.openagents/jev.json`. The client talks to TypeSafe directly.
2. Otherwise the hosted service. The client is an ordinary `jev::Client`
   whose attempts go through `jev_hosted::RelayExchange`; `base_url()` is
   still `https://api.typesafe.ai` (the door that answers, and what a grant
   pins), and `service()` names the worker and relay.
3. No Jev only when neither is available, with the reason:
   `OPENAGENTS_JEV_HOSTED=off`, or a door other than TypeSafe's.

A hosted call that cannot be answered fails with a sentence that says why:
`Jev is unreachable: the hosted decision service on wss://relay.openagents.com …`
or, only under an operator's emergency brake, `Jev refused: busy (This
worker can't take this job right now. Try again later.)`.
`jev_hosted::unavailable` reads that reason back, and a repository turn then
stops asking for the rest of its task and records `decision_unavailable`
once with the reason, instead of waiting on every step.

The callers:

| Caller | Where it resolves |
| --- | --- |
| Local runs (`openagents chat --local`) and the desktop's auto-start | `microcoder repository` (`crates/microcoder/src/main.rs`), pinned to the grant's `decision_endpoint` and `decision_model` |
| The delegate door (terminal and `coder -p` turns) | `coder::delegate_door::jev_from` through `coder_delegate::credentials::jev` |
| The hands seam | `coder_hands::seam::configured` (its key, `TYPESAFE_BASE_URL`, and model pass through the resolver) |
| Coder One's episode, checks, and tools | `coder_delegate::credentials::jev_live` |
| The Microcoder bench and gate replay | `microcoder`'s `jev_client` |
| The decision profile (terminal routing, the shell judge, `decide` steps, the chat and CLI routers) | `coder::profiles::Profile::client`: keyed doors through the local-key door, the `relay` profile through the hosted door |
| The Gym (`gym ask --jev`, run learning) | `gym`'s `open_doors` and `runs_learning::Judge::from_environment` |
| External-eval graders (the eval runner and `openagents eval`) | `ext_eval::JevDoor::resolved` |
| Voyager's live door | `voyager::decide::Door::live` |

Every one records its calls the same way when it keeps a trajectory
(`jev_hosted::decision_record`, `openagents.decision-call.v1`): the door,
`via`, the relaying `service`, latency, usage, and cost.

Each repository turn's transcript names the service once
(`decision_service`: `via` `direct` or `hosted`), and each decision response
names its door, `via`, the worker's `service`, and its milliseconds.
`OPENAGENTS_JEV_RELAY` and `OPENAGENTS_JEV_WORKER` point clients at another
relay and worker, for a fixture or staging.

## Bounds and usage

The open lane has no usage limit. The owner decided on 2026-10-01
("ensure no such limits anywhere in the app, I never want to see a limit
again … just record all the info so later we can pull usage stats
easily"; [#10120](https://github.com/OpenAgentsInc/openagents/issues/10120),
[#10121](https://github.com/OpenAgentsInc/openagents/issues/10121)), so it
answers every caller key with no per-minute, per-day, or total count, the
chat worker's design. What bounds a job is its shape:

| Bound | Deployed value | Refusal |
| --- | --- | --- |
| Request ciphertext | 256 KiB | `limit_exceeded` |
| Models | `jev-1.13.0`, `jev-latest`, and the alias `typesafe/jev-1.13` | `not_admitted` |
| Jobs at once | 8 | `busy` |

- **The emergency brake.** For an abuse emergency only, an operator may
  add `per_key_minute`, `per_key_day`, or `total_day` to `open.quota` in
  the release's `decision-worker.json` and restart. The deployed config
  names none, and the journal's start line says `open lane under
  $TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, no usage limit`. A set
  count refuses `rate_limited` or `quota_exhausted` with `retry_after_ms`
  and a message that names no count ("This worker can't take this job
  right now. Try again later."); the day's counts are in
  `/var/lib/decision-worker/quota.json`, so a restart keeps the day, and a
  file that exists but does not read stops the worker.
- The dollar bound is on the door keys themselves (the AI Gateway key's
  cap, `NEEDS_OWNER.md`), not on callers. At Jev's list price ($0.042 per
  million input tokens) a 10,000-token judgment costs about $0.0004.
- A redelivery of a settled `(request, attempt)` pair republishes the
  recorded result from `jobs.jsonl` and is not a new job.

**The usage log.** Every decision job the worker reads, answered or
refused, is one JSON line in `/var/lib/decision-worker/usage/YYYY-MM-DD.jsonl`
(the UTC day it arrived): time, caller key, lane (`open`, `principal`,
`anonymous`), request id and attempt, the model asked and the model that
answered, the door that answered, the doors' and the whole job's time,
tokens and cost when the door reported them, outcome and code, and the
request's size. No state, question, or key. On `oa-coder-worker-1`:

```sh
sudo /opt/decision-worker/current/decision-worker usage                 # by day
sudo /opt/decision-worker/current/decision-worker usage --by key        # per caller key
sudo /opt/decision-worker/current/decision-worker usage --by door --since 2026-10-01
sudo /opt/decision-worker/current/decision-worker usage --by outcome --json
sudo sh -c 'cat /var/lib/decision-worker/usage/*.jsonl' | jq -s 'map(.cost_usd // 0) | add'
```

`--by` takes `key`, `lane`, `model`, `door`, `day`, or `outcome`; each
row has jobs, answered, refused, failed, distinct keys, input tokens,
cost, and the median time of an answered job. `--dir` or
`DECISION_WORKER_USAGE_DIR` reads another directory.

`INVARIANTS.md` (Hosted decision service) records these with their tests.

## Configuration

The worker is `decision-worker <config.json>` from `crates/gateway`
([operator's guide](../decision-models/service/decision-worker.md)). The
deployed config is [`deploy/decision-worker/decision-worker.json`](../../deploy/decision-worker/decision-worker.json):
`open` is the open lane (`key_env` names the variable holding the door
key; `models`; `quota`, the request size and an emergency brake that is
off), `service` is what each answer names, and
`probe_secs` is the liveness probe: a `REQ` with `limit` 0 every 30 seconds,
and one still unanswered at the next ends the session, which reconnects.
The environment file ([example](../../deploy/decision-worker/decision-worker.env.example))
holds the two secrets, `DECISION_WORKER_SECRET` and `TYPESAFE_API_KEY`, and
optionally `AI_GATEWAY_API_KEY` and `OPENROUTER_API_KEY`, which turn the
backup doors on.

## Deploying

The worker runs on the Coder worker VM (`oa-coder-worker-1`, us-central1-a)
under its own unit, `decision-worker.service`, and its own release
directory, `/opt/decision-worker/releases/<commit>` with the
`/opt/decision-worker/current` symlink. It never touches
`coder-worker.service`, `coder-worker-chat.service`, or `/opt/coder-worker`.
The unit runs under `DynamicUser=yes` with a `StateDirectory`, so there is
no account to create.

Build a static binary on a development Mac from a commit rebased on
`origin/main`:

```sh
cargo zigbuild --locked --release -p gateway --bin decision-worker \
  --target x86_64-unknown-linux-musl
```

Copy it, the config, the unit, and the filled environment file into a
private directory on the VM with `gcloud compute scp --tunnel-through-iap`
(`CLOUDSDK_CONFIG` set to the automation account's config), then on the VM,
with `<VERSION>` the short commit:

```sh
sudo install -d -m 0755 /opt/decision-worker/releases/<VERSION>
sudo install -m 0755 decision-worker /opt/decision-worker/releases/<VERSION>/decision-worker
sed "s/RELEASE/<VERSION>/" decision-worker.json | sudo tee /opt/decision-worker/releases/<VERSION>/decision-worker.json >/dev/null
sudo ln -sfn /opt/decision-worker/releases/<VERSION> /opt/decision-worker/current
sudo install -d -m 0700 /etc/decision-worker
sudo install -m 0600 decision-worker.env /etc/decision-worker/decision-worker.env
sudo install -m 0644 decision-worker.service /etc/systemd/system/decision-worker.service
sudo systemctl daemon-reload
sudo systemctl enable --now decision-worker
sudo journalctl -u decision-worker -n 20 --no-pager
```

The first lines must name the pubkey `ad6b4d91…`, the upstream
`https://api.typesafe.ai`, the open lane's `no usage limit`, its door
order, and the usage log. The worker secret is
kept with the owner's secrets as `decision-worker.env`; the TypeSafe key is
the owner's (`typesafe.env`).

To upgrade, install the new release beside the old one, move the `current`
symlink, and `sudo systemctl restart decision-worker`. To roll back, move
the symlink back and restart.

## Checking it

From a checkout, on a computer with no TypeSafe key, in a temporary `HOME`:

```sh
env -u TYPESAFE_API_KEY HOME=$(mktemp -d) \
  cargo test -p jev-hosted --test live -- --ignored --nocapture
```

`live_hosted_decision_answers` asks the deployed worker one judgment and
prints the answer, its `service`, and its time.
`live_hosted_structured_decision_answers` asks a structured
([NIP-DEC](../../nips/openagents/NIP-DEC.md)) decision: an object `state`, a
`choice` whose options are `what` / `not_for` / `examples` rubrics, and a
`noul` with structured `true` and `false`. A Coder run is the real
check: `openagents chat --local "add a unit test for <fn>"` in a scratch
checkout with no key should record `decision_service` with `via: hosted`
and `decision_response` steps whose `service` names the worker, and no
"no Jev key" line.

### When Jev is unreachable

A transcript that says `Jev is unreachable: the hosted decision service on
wss://relay.openagents.com did not answer within … s` means the worker is not
answering. On `oa-coder-worker-1`:

1. `sudo journalctl -u decision-worker --since -15min --no-pager`. A
   healthy worker is quiet between jobs; `session ended … reconnecting`
   lines mean the relay dropped it and it is rejoining (the relay's Cloud
   Run request timeout drops every connection within the hour). Check the
   relay (`openagents-nostr-relay`, [runbook-cloud-run.md](runbook-cloud-run.md))
   if they repeat.
2. `open lane refused … quota_exhausted` lines appear only when an
   operator set an emergency brake; the transcript says `Jev refused:
   busy`. Take the counts out of `open.quota` and restart to lift it.
3. `sudo systemctl restart decision-worker` and run the live check.

## Deploy record

Release `04113fec9d` (2026-09-30,
[#10044](https://github.com/OpenAgentsInc/openagents/issues/10044)) is the
first deploy. It was built with `cargo zigbuild` as above, installed as
`/opt/decision-worker/releases/04113fec9d` with `current` pointing at it,
and started as `decision-worker.service`; the environment file holds a new
worker secret (kept as the owner's `decision-worker.env`) and the owner's
TypeSafe key. `coder-worker.service`, `coder-worker-chat.service`, and
`/opt/coder-worker` were not touched. The journal names pubkey
`ad6b4d91…`, upstream `https://api.typesafe.ai`, and `open lane under
$TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, quota 2000/key/day
60/key/min 20000/day total`.

Live checks the same day, each in a temporary `HOME` with the owner's
Codex login linked in and a scratch Python checkout in `$TMPDIR`:

- `live_hosted_decision_answers` with no key: `model jev-1.13.0
  passed=0.990 service={"door":"https://api.typesafe.ai","version":"decision-worker@04113fec9d"}
  input_tokens=Some(286) in 673 ms`.
- `openagents chat --local "add a unit test for slugify"` with no key: the
  run recorded `decision_service` `via: hosted` and six `decision_response`
  steps answered by the deployed worker (650 to 1,208 ms each, 735 to 2,012
  input tokens), and no "no Jev key" line. The worker's quota file counted
  the jobs against the run's decision key.
- The same run against a second worker with a two-job day (run on the
  development Mac against the production relay, so production limits did
  not change): two answers, then `Coder runs without Jev's judgments for
  the rest of this task: Jev refused: quota (This key used today's decision
  jobs on this worker.)`, and the task finished.
- With `TYPESAFE_API_KEY` set: `decision_service` `via: direct`, seven
  direct answers (130 to 889 ms), and no decision key made.

NIP-DEC ([#10047](https://github.com/OpenAgentsInc/openagents/issues/10047),
2026-09-30) needed no redeploy: release `04113fec9d` already forwards
structured `state` and questions to TypeSafe unchanged.
`live_hosted_structured_decision_answers` against it, with no key:
`model jev-1.13.0 queue=billing confidence=1.000 probabilities={"billing":
1.0, "sales": 0.0, "technical": 0.0} refund=0.990
service={"door":"https://api.typesafe.ai","version":"decision-worker@04113fec9d"}
input_tokens=Some(677) in 553 ms`. The `typesafe/jev-1.13` alias and the
HTTP-status reading of an untyped door error arrive with the next release.

Release `5710e1311c` (2026-09-30,
[#10048](https://github.com/OpenAgentsInc/openagents/issues/10048), "NIP-DEC
everywhere") replaced it: built with `cargo zigbuild` as above, installed as
`/opt/decision-worker/releases/5710e1311c` beside `04113fec9d`, `current`
moved to it, and `decision-worker` restarted. The environment file was not
changed, so the backup door is off; `coder-worker.service`,
`coder-worker-chat.service`, and `/opt/coder-worker` were not touched. The
journal:

```text
decision-worker: pubkey ad6b4d9199bf0864b1a402116d44e36daa1a72c6f8df4ae07bd57c8df5c922fc
decision-worker: upstream https://api.typesafe.ai
decision-worker: relay wss://relay.openagents.com
decision-worker: open lane under $TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, quota 2000/key/day 60/key/min 20000/day total
decision-worker: backup door https://openrouter.ai/api/alpha/decisions off: $OPENROUTER_API_KEY is not set
```

Live checks the same day, with no key, in a temporary `HOME`:

- `live_hosted_docs_examples_answer_under_the_alias`: all five TypeSafe
  docs examples (`crates/gateway/tests/fixtures/typesafe-docs`) asked under
  `typesafe/jev-1.13`, each answered as `jev-1.13.0` by
  `decision-worker@5710e1311c` in 509 to 558 ms, recorded `via: hosted`
  with `cost_usd` (for example the invoice battery: `customer_name` chose
  `Beaver Dam Logistics` at 1.0, `amount_due` level 2, `payment_terms` Net
  30, 0.0000335 USD); `live_hosted_structured_decision_answers` and
  `live_hosted_decision_answers` answered as before.
- `openagents chat --local "add a unit test for slugify"` in a scratch
  checkout, with the owner's Codex login linked in: the task finished
  (`test_slugs.py`, +15), and the thread's exported ATIF holds five
  `openagents.decision-call.v1` calls from the repository adapter
  (`openagents.microcoder.judge.v1`), each with the structured request (an
  object `state` and three nouls), `via: hosted`, `service`
  `decision-worker@5710e1311c`, 677 to 912 ms, usage, and `cost_usd`, beside
  the chat router's call; no "no Jev key" line.

Release `5ed35bf130` (2026-10-01 UTC,
[#10064](https://github.com/OpenAgentsInc/openagents/issues/10064), "Jev
fallback doors") replaced it: built with `cargo zigbuild` as above,
installed as `/opt/decision-worker/releases/5ed35bf130` beside
`5710e1311c` and `04113fec9d` (rollback: move `current` back and restart),
with the `backups` config (the Vercel AI Gateway, then OpenRouter). The
owner's new OpenRouter key was added to the environment file with
`scripts/decision-worker-install-door-keys.sh`, which restarted the worker;
there is no AI Gateway key yet, so that door is off. `coder-worker.service`
and `/opt/coder-worker/current` were not touched. The journal:

```text
decision-worker: pubkey ad6b4d9199bf0864b1a402116d44e36daa1a72c6f8df4ae07bd57c8df5c922fc
decision-worker: upstream https://api.typesafe.ai
decision-worker: relay wss://relay.openagents.com
decision-worker: open lane under $TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, quota 2000/key/day 60/key/min 20000/day total
decision-worker: backup door https://ai-gateway.vercel.sh/typesafe/v1/systemone off: $AI_GATEWAY_API_KEY is not set
decision-worker: backup door https://openrouter.ai/api/alpha/decisions under $OPENROUTER_API_KEY
```

Live checks the same night, with no key, in a temporary `HOME`:
`live_hosted_decision_answers` answered `jev-1.13.0 passed=0.990
service={"door":"https://api.typesafe.ai","version":"decision-worker@5ed35bf130"}`
in 590 ms and `live_hosted_structured_decision_answers` `queue=billing
confidence=1.000 refund=0.990` in 657 ms: TypeSafe answers again (the
owner added credits), so the backup doors were not asked. The OpenRouter
door was checked live without breaking TypeSafe, from the development Mac
with the same key: `live_worker_backup_door_answers_through_openrouter`
(`crates/gateway/tests/relay_worker.rs`; the deployed config with a closed
loopback port as the upstream) logged `upstream unavailable (unavailable);
backup door https://openrouter.ai answered answered` and the answer
`typesafe/jev-1.13-20260917`, `refund=0.99`, `department=billing`, 341
input tokens, `cost` 0.000014322 USD, in 332 ms; `jev`'s
`live_openrouter_door_answers_when_typesafe_cannot` (the client-side
failover the chat worker uses) answered the same in 374 ms with
`service.door` `https://openrouter.ai`.

The gateway door turns on once `AI_GATEWAY_API_KEY` is in
`~/work/.secrets/ai-gateway.env` and an agent runs
`scripts/decision-worker-install-door-keys.sh`; the owner step is in the
workspace's `NEEDS_OWNER.md`.

Release `aab51b62af` (2026-10-01,
[#10112](https://github.com/OpenAgentsInc/openagents/issues/10112), "the
gateway first, TypeSafe last") replaced it: built with `cargo zigbuild` as
above, installed as `/opt/decision-worker/releases/aab51b62af` beside the
earlier releases (rollback: move `current` back to `5ed35bf130` and
restart), with `open.upstream_last` in the config, so the open lane asks
the Vercel AI Gateway, then OpenRouter, then TypeSafe last, and benches a
401 or 402 door for five minutes. The owner's AI Gateway key was already
in the environment file; the file was backed up and not changed.
`coder-worker.service`, `coder-worker-chat.service`, and
`/opt/coder-worker` were not touched. The journal:

```text
decision-worker: pubkey ad6b4d9199bf0864b1a402116d44e36daa1a72c6f8df4ae07bd57c8df5c922fc
decision-worker: upstream https://api.typesafe.ai
decision-worker: relay wss://relay.openagents.com
decision-worker: open lane under $TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, quota 2000/key/day 60/key/min 20000/day total
decision-worker: open lane doors https://ai-gateway.vercel.sh → https://openrouter.ai → https://api.typesafe.ai
decision-worker: backup door https://ai-gateway.vercel.sh/typesafe/v1/systemone under $AI_GATEWAY_API_KEY
decision-worker: backup door https://openrouter.ai/api/alpha/decisions under $OPENROUTER_API_KEY
decision-worker: door https://ai-gateway.vercel.sh answered in 292 ms
```

Before it, TypeSafe direct was refusing every job with `payment_required`
and the gateway answered each one second (`upstream refused
(payment_required); backup door https://ai-gateway.vercel.sh answered`).

Live checks the same day, with no key, in a temporary `HOME`, through
`jev_hosted::resolve` with the TypeSafe door a host's autostart policy
names as `decision_endpoint` (the path `microcoder repository` takes):
`live_hosted_decision_answers` answered `typesafe-ai/jev passed=0.990
service={"door":"https://ai-gateway.vercel.sh","version":"decision-worker@aab51b62af"}`
in 856 ms; `live_hosted_structured_decision_answers` `queue=billing
confidence=1.000 refund=0.990` in 572 ms; and
`live_hosted_docs_examples_answer_under_the_alias` answered all five docs
examples from the gateway in 557 to 683 ms, recorded `via: hosted` with
`cost_usd` (0.000020 to 0.000034 USD). The journal logged ten `door
https://ai-gateway.vercel.sh answered` lines (192 to 484 ms at the worker)
and no other door. The live tests now accept the admitted Jev as the
answering door names it (`typesafe-ai/jev` at the gateway), as a
repository run's judge does (#10107).

Release `a3de5c8ff8` (2026-10-01,
[#10121](https://github.com/OpenAgentsInc/openagents/issues/10121), "no
usage limit; record every job") replaced it: built with `cargo zigbuild` as
above, installed as `/opt/decision-worker/releases/a3de5c8ff8` beside the
earlier releases (rollback: move `current` back to `aab51b62af` and
restart), with the deployed config's `open.quota` holding only
`max_request_bytes` and `file`. The environment file was not changed.
`coder-worker.service`, `coder-worker-chat.service`, and
`/opt/coder-worker` were not touched. The journal:

```text
decision-worker: open lane under $TYPESAFE_API_KEY, models jev-1.13.0,jev-latest, no usage limit
decision-worker: open lane doors https://ai-gateway.vercel.sh → https://openrouter.ai → https://api.typesafe.ai
decision-worker: usage log /var/lib/decision-worker/usage
```

Live proof at 21:37 UTC, from the development Mac with no TypeSafe key, in
a temporary `HOME` (one decision key, five jobs at a time through
`jev_hosted::resolve`): 75 of 75 jobs answered in 10.6 s, past the old 60
per key per minute, and the journal logged 75 `door
https://ai-gateway.vercel.sh answered` lines and no refusal.
`decision-worker usage --by key` on the VM:

```text
key                                                               jobs  answered  refused  failed  keys  tokens_in  cost_usd  total_p50_ms
401eca516ec6182fb7f3b8753df8e84bcc80468684de4bd6404693285cb0164d    75        75        0       0     1      21580  0.000906           202
```

Each line names the lane (`open`), the request and attempt, the model
asked (`jev-1.13.0`) and served (`typesafe-ai/jev`), the door, the door's
and the job's time, tokens, and cost (0.000012 USD a job).
