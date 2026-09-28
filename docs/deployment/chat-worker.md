# The chat worker: the OpenAgents app's basic Coder

A new OpenAgents app user can chat with Coder before connecting a computer.
That chat, the *basic Coder*, is served by one `coder-worker` running in
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
- **Streaming.** The worker sends its first delta at once and then about 160
  bytes at a time; the phone checks each answer's signer, recipient, request
  binding, and sequence, draws the reply with Rust Native's incremental
  Markdown, and replaces the preview with the result.

The alternatives were a new HTTPS endpoint with its own device
authentication, or the decision-API gateway (`crates/gateway`). Both need a
new credential flow for the phone. The relay door already authenticates by
the device key, already streams, and already has a deployed worker and
runbook, so the basic chat reuses it.

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
  40 seconds." or "Coder has answered all the messages it can today…".

`INVARIANTS.md` (Basic chat) records these as invariants with their tests.

## Deploying

The chat worker runs beside the executor worker on the Coder worker VM
(`oa-coder-worker-1`) from the same release directory, under its own unit and
environment file. Follow [`deploy/README.md`](../../deploy/README.md) for the
user, release directory, and binary, then:

```sh
sudo install -o root -g coder-worker -m 0640 deploy/coder-worker-chat.env.example \
  /etc/coder-worker/coder-worker-chat.env
sudoedit /etc/coder-worker/coder-worker-chat.env   # the worker secret and the gateway key
sudo systemd-run --pipe --wait -p User=coder-worker \
  -p EnvironmentFile=/etc/coder-worker/coder-worker-chat.env \
  /opt/coder-worker/current/coder-worker --check
sudo install -o root -g root -m 0644 deploy/systemd/coder-worker-chat.service \
  /etc/systemd/system/coder-worker-chat.service
sudo systemctl daemon-reload
sudo systemctl enable --now coder-worker-chat
sudo journalctl -u coder-worker-chat -n 20 --no-pager
```

The first log line must be `worker  32c07895…` (the key the app carries) and
the admits line must name the quota. The worker secret is kept with the
owner's secrets as `coder-chat-worker.env`; the gateway key is the owner's
AI Gateway key.

To check the deployed path from a checkout, send one turn from a fresh key,
as a new install does:

```sh
cargo test --manifest-path crates/openagents-mobile/Cargo.toml \
  live_basic_coder_streams_a_reply -- --ignored --nocapture
```

On 2026-09-28 that test answered through the production relay in 5.7 s,
with the first words at 5.1 s, from a worker with this configuration running
on a development machine.
