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
- **First response and the chat router.** The worker acknowledges every
  admitted turn with `status: processing` at once. With `TYPESAFE_API_KEY`
  set, a turn that asks gets one Jev (System One) judgment run beside the
  model call, the `chat-router-v1` question set (`crates/coder/src/router/`):
  route, prepared answer, whether the reply needs the user's specifics,
  lane, opener, risk, and a command group when a command tree is wired.
  Code decides what is shown. A turn that sends `"router":
  "chat-router-v1"` (build 19 of the app) gets every tier: a whole answer
  from the reviewed bank (`crates/coder/answers/chat-answers-v1.toml`) with
  followup chips, a refusal, a "We'll dispatch Coder to …" stem with a Run
  Coder or Connect a computer offer, a wallet or account answer with its
  screen offered, or the model led by an opener. A turn that sends only
  `"opener": true` (builds before 19) gets a whole answer with no offer, an
  opener, or nothing, as before. Every judged turn logs one `router` line
  of ids, probabilities, tiers, and the judge's time, never message text.
  `CODER_WORKER_ROUTER=shadow` logs what the router would serve but serves
  what `opener` alone would; `off` ignores `router`; unset is `live`. The
  worker refuses to start when the bank breaks its lint. See the
  [chat router design](../coder/design/2026-09-28-chat-router.md) and
  [`docs/coder/measurements/2026-09-28-first-reply.md`](../coder/measurements/2026-09-28-first-reply.md).
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
    The OpenRouter account must have credits: on 2026-09-28 it answered HTTP
    402, and every personalized stem would then close with its generic
    ending. `seam_from_env()` refuses to start without a key rather than
    run without it.
  - With it on, each personalized turn sends the route, the stem, and the
    user's latest message (redacted of secret shapes, at most 600
    characters) to that provider, so the privacy answer and the Basic chat
    rows in `INVARIANTS.md` must name it as a recipient (the seam's
    `recipients()`) in the change that turns it on.
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
