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
- **First response.** The worker acknowledges every admitted turn with
  `status: processing` at once. With `TYPESAFE_API_KEY` set, a turn that asks
  with `"opener": true` also gets one Jev (System One) judgment run beside the
  model call (`coder-first-response-v2`): the turn's route, whether it needs a
  computer (`lane`), which prepared answer from the `chat-answers-v1` bank
  fits, whether the reply needs the user's specifics, and which opener fits.
  A sure prepared answer ("Who are you?", "What model are you?", "hi",
  "thanks") is the whole reply in about half a second and the model call is
  dropped; otherwise a sure opener ("Here's how that works.") leads the
  model's reply; otherwise the model's own words come first. Every canned
  line speaks as OpenAgents in the plural. See `crates/coder/src/first.rs`
  and [`docs/coder/measurements/2026-09-28-first-reply.md`](../coder/measurements/2026-09-28-first-reply.md).
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
