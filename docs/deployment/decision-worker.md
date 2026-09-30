# The decision worker: Jev with no key on the computer

Coder asks Jev, TypeSafe's System One model, for its judgments. A computer
with a TypeSafe key asks TypeSafe directly. Every other computer asks the
*hosted decision service*: one `decision-worker` that holds the TypeSafe key
on the server and answers NIP-CJ decision jobs on `wss://relay.openagents.com`.
This page is the serving path, its limits, and the runbook.

## The serving path

```text
Coder (decision key) --NIP-CJ 25910, NIP-44--> relay.openagents.com --> decision worker
Coder <--27010 status, 26910 result----------- relay.openagents.com <-- decision worker --> api.typesafe.ai
```

- **Wire.** Each judgment is one `POST /v1/systemone` call carried as a
  NIP-CJ decision job ([`nips/openagents/NIP-CJ.md`](../../nips/openagents/NIP-CJ.md),
  "Typed decision jobs"; `crates/nostr/src/decision.rs`): a kind `25910`
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
- **The door.** The worker forwards each admitted job to
  `https://api.typesafe.ai/v1/systemone` with `Idempotency-Key` and
  `X-Attempt`, under `TYPESAFE_API_KEY` from its environment file on its
  host. No TypeSafe key ships to a user's computer.
- **Evidence.** Every answer the worker relays carries
  `service: {door: "https://api.typesafe.ai", version: "decision-worker@<release>"}`
  and a sealed execution receipt bound to the request's digest. The client
  checks the worker's signature, the request's event id, its own key, and
  the receipt before it believes an answer.

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
or `Jev refused: quota (This key used today's decision jobs on this worker.)`.
`jev_hosted::unavailable` reads that reason back, and a repository turn then
stops asking for the rest of its task and records `decision_unavailable`
once with the reason, instead of waiting on every step.

The callers:

| Caller | Where it resolves |
| --- | --- |
| Local runs (`openagents chat --local`) and the desktop's auto-start | `microcoder repository` (`crates/microcoder/src/main.rs`), pinned to the grant's `decision_endpoint` and `decision_model` |
| The delegate door (terminal and `coder -p` turns) | `coder::delegate_door::jev_from` through `coder_delegate::credentials::jev` |
| The hands seam | `coder_hands::seam::configured` |

Each repository turn's transcript names the service once
(`decision_service`: `via` `direct` or `hosted`), and each decision response
names its door, `via`, the worker's `service`, and its milliseconds.
`OPENAGENTS_JEV_RELAY` and `OPENAGENTS_JEV_WORKER` point clients at another
relay and worker, for a fixture or staging.

## Limits

The worker answers keys nobody provisioned, so its open lane is metered
(`gateway::open_quota`), the chat worker's design:

| Limit | Deployed value | Refusal |
| --- | --- | --- |
| Jobs per caller key in any 60 seconds | 60 | `rate_limited`, with `retry_after_ms` |
| Jobs per caller key per UTC day | 2,000 | `quota_exhausted`, with `retry_after_ms` to midnight UTC |
| Jobs for every caller together per UTC day | 20,000 | `quota_exhausted`, with `retry_after_ms` to midnight UTC |
| Request ciphertext | 256 KiB | `limit_exceeded` |
| Models | `jev-1.13.0`, `jev-latest` | `not_admitted` |
| Jobs at once | 8 | `busy` |

- The total is the spend bound: a caller can mint any number of keys. At
  Jev's list price ($0.042 per million input tokens) a 10,000-token
  judgment costs about $0.0004, so the daily total bounds the day near $8.
- A job counts when it is admitted, whether or not the door answers; a
  refused job counts nothing and reaches no door. The day's counts are in
  `/var/lib/decision-worker/quota.json`, so a restart keeps the day; a file
  that exists but does not read stops the worker.
- A redelivery of a settled `(request, attempt)` pair republishes the
  recorded result from `jobs.jsonl` and is not counted again.

`INVARIANTS.md` (Hosted decision service) records these with their tests.

## Configuration

The worker is `decision-worker <config.json>` from `crates/gateway`
([operator's guide](../decision-models/service/decision-worker.md)). The
deployed config is [`deploy/decision-worker/decision-worker.json`](../../deploy/decision-worker/decision-worker.json):
`open` is the metered lane (`key_env` names the variable holding the door
key; `models`; `quota`), `service` is what each answer names, and
`probe_secs` is the liveness probe: a `REQ` with `limit` 0 every 30 seconds,
and one still unanswered at the next ends the session, which reconnects.
The environment file ([example](../../deploy/decision-worker/decision-worker.env.example))
holds the two secrets, `DECISION_WORKER_SECRET` and `TYPESAFE_API_KEY`.

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
`https://api.typesafe.ai`, and the open lane's quota. The worker secret is
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
prints the answer, its `service`, and its time. A Coder run is the real
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
2. `open lane refused … quota_exhausted` lines mean callers are spending
   the day; the transcript says `Jev refused: quota`.
3. `sudo systemctl restart decision-worker` and run the live check.
