# Run your own OpenAgents

Everything that runs openagents.com is in this repository, under the
Apache 2.0 license ([`LICENSE`](../LICENSE)). This guide runs your own copy
from it, with your own model keys: the website, the account service and
inference gateway, the chat worker that writes the answers, and, if you
want, your own Nostr relay.

The short web version is [openagents.com/docs/self-host](https://openagents.com/docs/self-host).

**Status (2026-10-09).** Written from the code, not yet followed start to
finish on a clean Linux machine. That run is the open half of #11133. If a
step fails, open an issue with the step and the output.

## The parts

| Part | Built from | What it does | Needed |
| --- | --- | --- | --- |
| Website | `crates/openagents-web` (`openagents-web`) | The pages, the chat, sign-in, Settings, and the API Coder syncs chats through. | Yes |
| Account service and inference gateway | `crates/gateway` (`gateway`), one process | GitHub sign-in, accounts and sessions; every model call from the chat worker, sent on to your model provider keys. | For sign-in. The script below always runs it. |
| Chat worker | `crates/coder` (`coder-worker`) | Picks up each chat message, asks the model, and sends the answer back. | Yes |
| Nostr relay | `crates/nostr-relay` (`nostr-relay`) | Carries messages between the website and the chat worker. | No. By default your website and worker meet on `wss://relay.openagents.com`; each message is encrypted for your worker, so that relay only passes it along. Run your own with step 5. |

The website and the chat worker never talk directly: the website posts an
encrypted job on the relay naming the worker's public key, and the worker
answers on the same relay ([NIP-CJ](../nips/openagents/NIP-CJ.md)).

## What you need

- macOS or Linux. The commands below use Debian or Ubuntu package names.
- Rust through [rustup](https://rustup.rs). The repository pins Rust
  1.97.1 in `rust-toolchain.toml`; rustup installs it on the first build.
- Build tools and the small programs the start script uses:

  ```sh
  sudo apt-get install -y build-essential pkg-config git curl openssl lsof
  ```

- An [OpenRouter](https://openrouter.ai/keys) API key. Chat answers go
  through the gateway, whose chat route tries Google Vertex, then the
  OpenAgents Pro door, then OpenRouter (`crates/inference/src/router.rs`).
  Without Google credentials or a Pro key, OpenRouter answers. A Google
  Cloud service account with Vertex AI also works
  (`GOOGLE_APPLICATION_CREDENTIALS`, `VERTEX_PROJECT`, `VERTEX_LOCATION`).
- A GitHub OAuth App for sign-in. Create it at
  [github.com/settings/applications/new](https://github.com/settings/applications/new)
  with homepage `http://127.0.0.1:4301` and callback
  `http://127.0.0.1:4301/auth/github/callback`, then generate a client
  secret.

Optional keys:

- `TYPESAFE_API_KEY`: a quick first line before the model's answer,
  follow-up suggestions, and answers about OpenAgents itself from the
  `knowledge/openagents` folder.
- `AI_GATEWAY_API_KEY` (Vercel AI Gateway): the search behind those
  answers about OpenAgents. Unless both keys are set, the model answers
  those questions on its own.

## 1. Get the code

```sh
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

## 2. Write your private files

The start script reads private files from `$OPENAGENTS_SECRETS` (default
`~/work/.secrets`). Keep the folder private and never commit it.

```sh
export OPENAGENTS_SECRETS="$HOME/.openagents-secrets"
mkdir -p "$OPENAGENTS_SECRETS" && chmod 700 "$OPENAGENTS_SECRETS"
umask 077

# Your GitHub OAuth App, plus a key that encrypts GitHub tokens at rest.
cat > "$OPENAGENTS_SECRETS/github-oauth-local.json" <<EOF
{"client_id": "<CLIENT_ID>", "client_secret": "<CLIENT_SECRET>", "token_encryption_key": "$(openssl rand -base64 32)"}
EOF

# The key that encrypts Claude keys people save in Settings.
cat > "$OPENAGENTS_SECRETS/openagents-web-cloud-byo-keys.json" <<EOF
{"schema": "openagents.seal.keyring.v1", "current": "k1", "keys": {"k1": "$(openssl rand -base64 32)"}}
EOF

# Your model key.
echo 'OPENROUTER_API_KEY=<YOUR_OPENROUTER_KEY>' > "$OPENAGENTS_SECRETS/openrouter.env"
```

Replace `<CLIENT_ID>`, `<CLIENT_SECRET>`, and `<YOUR_OPENROUTER_KEY>`. The
optional keys go in `typesafe.env` (`TYPESAFE_API_KEY=...`) and
`ai-gateway.env` (`AI_GATEWAY_API_KEY=...`) in the same folder. A key
already exported in your shell wins over its file.

## 3. Build and start everything

```sh
scripts/dev/full-local.sh start
```

This builds the four binaries and starts them, all on `127.0.0.1`
([`scripts/dev/full-local.sh`](../scripts/dev/full-local.sh), more in
[`docs/dev/full-local.md`](dev/full-local.md)). The first build compiles
the whole workspace and takes a while; later starts take about two minutes,
or seconds with `--no-build`. Build output goes to `$CARGO_TARGET_DIR`
(default `~/work/openagents-target-fulllocal`).

| What | Where |
| --- | --- |
| Website | <http://127.0.0.1:4301> (GitHub sign-in only comes back to this port) |
| Gateway | `127.0.0.1:8791`; usage dashboard at `/admin/inference` with the token in `~/.openagents/full-local/admin.token` |
| Chat worker | Joins the relay with its own new key; the website is told that key |
| Accounts, chats, saved keys | `~/.openagents/full-local/` (`$FULL_LOCAL` moves it) |
| Logs | `~/.openagents/full-local/logs/{gateway,worker,web}.log` |

The script also tries the browser builds of the chat composer, the
component catalog, and Grow Little Bunny. Those builds use tools a fresh
machine won't have (the `openagents` command, the `wasm32-unknown-unknown`
target, and `wasm-bindgen` at the version in `Cargo.lock`). When they fail,
the site starts without them: the composer and the catalog fall back to
plain HTML, and the game page can't be played.

`scripts/dev/full-local.sh status` shows what runs;
`scripts/dev/full-local.sh stop` stops it.

## 4. Check it works

1. Open <http://127.0.0.1:4301> and send a message. An answer streams in.
2. Click **Log in**, then **Continue with GitHub**, and approve. You come
   back signed in, with your chats moved to your account.
3. To use Coder in the terminal against your copy, run
   `~/.openagents/full-local/bin/coder login`. It is this checkout's Coder,
   pointed at your website with its own sign-in and chats.

If the answer never comes, read `logs/worker.log`: its first lines name the
relay, the model route, and anything missing.

## 5. Run your own relay (optional)

The relay needs Postgres. It creates its tables on first start.

```sh
sudo apt-get install -y postgresql
sudo -u postgres createuser --pwprompt nostr-relay
sudo -u postgres createdb --owner=nostr-relay nostr-relay
cargo build --release -p nostr-relay --bin nostr-relay
DATABASE_URL='postgres://nostr-relay:<DB_PASSWORD>@127.0.0.1:5432/nostr-relay' \
NOSTR_RELAY_PORT=8080 \
  "${CARGO_TARGET_DIR:-target}/release/nostr-relay"
```

It listens on `127.0.0.1:8080` (`curl http://127.0.0.1:8080/health`). Point
the website and the chat worker at it:

```sh
scripts/dev/full-local.sh stop
FULL_LOCAL_RELAY=ws://127.0.0.1:8080 scripts/dev/full-local.sh start --no-build
```

Every relay setting is in
[`docs/deployment/configuration.md`](deployment/configuration.md).

## Each part on its own

The script is the exact wiring; read its `start` function to run the parts
separately or under your own service manager. In short:

| Part | Build | Run |
| --- | --- | --- |
| Relay | `cargo build --release -p nostr-relay --bin nostr-relay` | `DATABASE_URL=... nostr-relay` |
| Gateway | `cargo build --release -p gateway --bin gateway -p tenancy --example bootstrap_registry` | `bootstrap_registry` once to make the account registry and the worker's service key, then `gateway --config gateway.json` |
| Chat worker | `cargo build --release -p coder --bin coder-worker` | `CODER_WORKER_SECRET=$(openssl rand -hex 32) CODER_RELAY=<relay> CODER_WORKER_OPEN=1 CODER_INFERENCE_URL=<gateway> CODER_INFERENCE_KEY=<service key> coder-worker`; it prints its public key on the `worker` line |
| Website | `cargo build --release -p openagents-web --bin openagents-web` | `OPENAGENTS_WEB_CHAT_WORKER=<worker public key> OPENAGENTS_WEB_CHAT_RELAY=<relay> openagents-web --listen 127.0.0.1:4301 --chat-store <dir> --cloud-config cloud.json --github-oauth github-oauth.json` |

What is optional:

- **No gateway.** The worker can call a provider itself: set
  `CODER_DOOR_KEY` to a Vercel AI Gateway key and/or `OPENROUTER_API_KEY`
  instead of `CODER_INFERENCE_*`, as
  [`deploy/coder-worker-chat.env.example`](../deploy/coder-worker-chat.env.example)
  does. The website then runs without `--cloud-config` and
  `--github-oauth`: chat works, sign-in doesn't.
- **No sign-in.** Without `--cloud-config`, visitors chat without
  accounts; their chats are tied to a browser cookie.
- **No saved Claude keys.** Leave out `--cloud-byo` and
  `--cloud-byo-keys`.

## What this setup leaves out

- **Projects (Connect GitHub)** needs a GitHub App as well as the OAuth
  App, passed with `--github-app`; the script doesn't set one up. See
  [`docs/auth/github.md`](auth/github.md).
- **Environments** stay off unless a Boat key and a Codex login are
  present. They start paid cloud machines.
- **Plans and payments.** Settings shows the Pro plan with "Subscribing
  isn't open on this server yet."

## Putting it on a public server

The script binds everything to `127.0.0.1` and is for one machine. For a
public host, the code requires today:

- **HTTPS in front.** Pass your host name with `--public-host`; the website
  then sets secure cookies.
- **Chats in a Google Cloud Storage bucket.** A public website refuses to
  start with chats on local disk: pass `--chat-bucket` (or
  `OPENAGENTS_WEB_CHAT_BUCKET`).
- **`OPENAGENTS_WEB_ASK_SALT`**, 64 hex characters
  (`openssl rand -hex 32`), shared by every copy of the website.
- **A GitHub OAuth App for your domain**, callback
  `https://<your host>/auth/github/callback`, set as `redirect_url` in the
  gateway config and as `public_origin` in the website's cloud config.
  Set `"open_signup": false` in the gateway config; the script turns it on
  so local tests can make accounts without GitHub.

The runbooks for each part on a server:

- Relay: [`docs/deployment/runbook-debian-vps.md`](deployment/runbook-debian-vps.md)
  (Debian, Postgres, systemd, Caddy or nginx for TLS).
- Chat worker: [`deploy/README.md`](../deploy/README.md) and
  [`docs/deployment/chat-worker.md`](deployment/chat-worker.md).
- Gateway: [`docs/decision-models/service/deployment.md`](decision-models/service/deployment.md).
- Website: [`docs/deployment/openagents-web.md`](deployment/openagents-web.md)
  describes how openagents.com runs on Google Cloud Run. There is no
  written guide yet for a public website off Google Cloud.
