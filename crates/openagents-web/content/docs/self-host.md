# Run your own OpenAgents

Everything that runs openagents.com is open source under the Apache 2.0
license, in [one repository](https://github.com/OpenAgentsInc/openagents).
You can run your own copy with your own model keys: the website, the
account service, the chat worker that writes the answers, and, if you want,
your own relay.

This guide hasn't yet been followed start to finish on a clean Linux
machine. If a step fails, [open an issue](https://github.com/OpenAgentsInc/openagents/issues/new)
with the step and what it printed. The full guide, with every option, is
[docs/self-host.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/self-host.md).

## The parts

| Part | What it does | Needed |
| --- | --- | --- |
| Website | Pages, chat, sign-in, Settings | Yes |
| Account service | GitHub sign-in and accounts; sends the chat worker's model calls to your keys | For sign-in |
| Chat worker | Answers each chat message with a model | Yes |
| Relay | Carries encrypted messages between the website and the chat worker | No: by default yours meet on relay.openagents.com, which only passes them along |

## What you need

- macOS or Linux, and Rust from [rustup.rs](https://rustup.rs).
- On Debian or Ubuntu, the build packages:
  `sudo apt-get install -y build-essential pkg-config git curl openssl lsof`
- An [OpenRouter API key](https://openrouter.ai/keys) for the answers.
- A [GitHub OAuth App](https://github.com/settings/applications/new) for
  sign-in, with homepage `http://127.0.0.1:4301` and callback
  `http://127.0.0.1:4301/auth/github/callback`.

## Start it

Get the code:

```sh
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

Write your private files. Replace the three values in angle brackets:

```sh
export OPENAGENTS_SECRETS="$HOME/.openagents-secrets"
mkdir -p "$OPENAGENTS_SECRETS" && chmod 700 "$OPENAGENTS_SECRETS"
umask 077
cat > "$OPENAGENTS_SECRETS/github-oauth-local.json" <<EOF
{"client_id": "<CLIENT_ID>", "client_secret": "<CLIENT_SECRET>", "token_encryption_key": "$(openssl rand -base64 32)"}
EOF
cat > "$OPENAGENTS_SECRETS/openagents-web-cloud-byo-keys.json" <<EOF
{"schema": "openagents.seal.keyring.v1", "current": "k1", "keys": {"k1": "$(openssl rand -base64 32)"}}
EOF
echo 'OPENROUTER_API_KEY=<YOUR_OPENROUTER_KEY>' > "$OPENAGENTS_SECRETS/openrouter.env"
```

Build and start everything. The first build takes a while:

```sh
scripts/dev/full-local.sh start
```

Then open [127.0.0.1:4301](http://127.0.0.1:4301) on the same machine and
send a message. Click **Log in** to sign in with GitHub. Stop everything
with `scripts/dev/full-local.sh stop`.

To use Coder in the terminal with your copy, run
`~/.openagents/full-local/bin/coder login`.

## Your own relay

The relay needs Postgres. Build it and start it on port 8080:

```sh
sudo apt-get install -y postgresql
sudo -u postgres createuser --pwprompt nostr-relay
sudo -u postgres createdb --owner=nostr-relay nostr-relay
cargo build --release -p nostr-relay --bin nostr-relay
DATABASE_URL='postgres://nostr-relay:<DB_PASSWORD>@127.0.0.1:5432/nostr-relay' \
NOSTR_RELAY_PORT=8080 \
  "${CARGO_TARGET_DIR:-target}/release/nostr-relay"
```

Then restart the rest on it:

```sh
scripts/dev/full-local.sh stop
FULL_LOCAL_RELAY=ws://127.0.0.1:8080 scripts/dev/full-local.sh start --no-build
```

## What's not covered yet

- Projects (Connect GitHub), Environments, and paid plans need more setup
  than this; the [full guide](https://github.com/OpenAgentsInc/openagents/blob/main/docs/self-host.md#what-this-setup-leaves-out)
  says what each one needs.
- This setup serves one machine. A public server also needs HTTPS and a
  Google Cloud Storage bucket for chats; the
  [full guide](https://github.com/OpenAgentsInc/openagents/blob/main/docs/self-host.md#putting-it-on-a-public-server)
  lists what the code asks for.

Next: [FAQ](/docs/faq).
