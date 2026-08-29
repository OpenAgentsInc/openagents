# Coder dev server test runbook

This runbook reproduces the test path used to check `openagents coder` against the local Phoenix backend, the local Rust `openagents-coder-api`, and the local `pro` gateway in `rc.8`. It also records the blockers found so that future agents do not repeat the same dead-ends.

## Goal

Verify that the Coder CLI can authenticate, open a thread, and stream a model response from a local dev server without relying on production.

## Repositories and binaries

- `openagents.com` (Phoenix) at `/Users/christopherdavid/work/openagents.com`
- `openagents` (Rust CLI) at `/Users/christopherdavid/work/openagents`
- `pro` (OpenAI gateway) at `/Users/christopherdavid/work/pro`
- `openagents-coder-api` is built from `openagents`:
  `/Users/christopherdavid/work/openagents/target/debug/openagents-coder-api`
- `pro` binary:
  `/Users/christopherdavid/work/pro/target/debug/pro`

## 1. Phoenix dev server (`openagents.com`)

This path tests the full Elixir stack with `glm-5.3-flash`, `gemini-3.7-flash`, and `openrouter/free`.

### Start the server

```sh
cd /Users/christopherdavid/work/openagents.com
source .env
mix ecto.migrate   # if the server returned 503 from PendingMigrationError
mix phx.server
```

The server runs on `http://localhost:4000`.

### Create a local API token

Phoenix `GET /api/v1/models` needs a token with `chat:account` scope. The fastest way is to insert a dev user and token directly:

```sh
USER_ID=$(psql -U "$(whoami)" -d openagents_dev -At -c "
  INSERT INTO users (
    id, github_id, github_login, github_name, github_avatar_url,
    status, github_token_scopes, notification_email_code_attempts,
    credit_allowance_microusd, inserted_at, updated_at
  ) VALUES (
    gen_random_uuid(), floor(random()*1000000000)::bigint,
    'dev-' || floor(random()*1000000)::int::text, 'Dev', '',
    'active', ARRAY[]::varchar[], 0, 20000000, now(), now()
  ) RETURNING id;
" | head -n 1)

TOKEN_ID=$(psql -U "$(whoami)" -d openagents_dev -At -c "SELECT gen_random_uuid();" | head -n 1)
TOKEN_SECRET=$(openssl rand -base64 32 | tr -d '=' | tr '+/' '-_')
PLAINTEXT="oa_pat_${TOKEN_ID}.${TOKEN_SECRET}"

psql -U "$(whoami)" -d openagents_dev -c "
  INSERT INTO api_tokens (
    id, user_id, name, token_digest, scopes, expires_at, inserted_at, updated_at
  ) VALUES (
    '${TOKEN_ID}', '${USER_ID}', 'dev-test',
    digest('${PLAINTEXT}', 'sha256'),
    ARRAY['chat:account'], now() + interval '1 day',
    now(), now()
  );
"

export OPENAGENTS_API_URL=http://localhost:4000
export OPENAGENTS_API_KEY="$PLAINTEXT"
```

Store the token in a file instead of printing it. For example:

```sh
cat > /tmp/.openagents_dev_token <<EOF
export OPENAGENTS_API_URL=http://localhost:4000
export OPENAGENTS_API_KEY=$PLAINTEXT
EOF
```

### Verify the catalog

```sh
curl -s -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  "$OPENAGENTS_API_URL/api/v1/models" | python3 -m json.tool
```

Expected: a JSON object with `default` and `models`, and `glm-5.3-flash` listed as `available`.

### Open a thread and get a grant

```sh
THREAD=$(curl -s -X POST \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"objective":"hello","model":"glm-5.3-flash"}' \
  "$OPENAGENTS_API_URL/api/v1/threads")
echo "$THREAD" | python3 -m json.tool
```

Expected: a `200` with `thread.id` and `grant.token`.

### Stream from the inference proxy

```sh
GRANT_TOKEN=$(echo "$THREAD" | python3 -c 'import sys,json; print(json.load(sys.stdin)["grant"]["token"])')

curl -s -N \
  -H "Authorization: Bearer $GRANT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"glm-5.3-flash","messages":[{"role":"user","content":"hello"}]}' \
  "http://localhost:4000/api/inference/proxy"
```

Expected: SSE `data:` lines with `choices[0].delta.reasoning` or `content` and `model: glm-5.3-flash`.

This proves the server streams and the `local` path in `InferenceProxyController` works.

## 2. Local Rust `openagents-coder-api`

This is the local inference door that serves `flash` and `free` lanes without a full Phoenix setup. It reads `openagents.com/.env` for `AI_GATEWAY_API_KEY` and `OPENROUTER_API_KEY`.

### Start

```sh
OPENAGENTS_CODER_API_BIND=127.0.0.1:4010 \
  /Users/christopherdavid/work/openagents/target/debug/openagents-coder-api
```

### Verify the catalog

```sh
curl -s http://127.0.0.1:4010/api/v1/models | python3 -m json.tool
```

Expected: `glm-5.3-flash`, `gemini-3.7-flash`, and `openrouter/free`.

### Full turn by hand

```sh
THREAD=$(curl -s -X POST \
  -H "Content-Type: application/json" \
  -d '{"objective":"hello","model":"glm-5.3-flash"}' \
  http://127.0.0.1:4010/api/v1/threads)

GRANT_TOKEN=$(echo "$THREAD" | python3 -c 'import sys,json; print(json.load(sys.stdin)["grant"]["token"])')

curl -s -N \
  -H "Authorization: Bearer $GRANT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"glm-5.3-flash","messages":[{"role":"user","content":"hello"}]}' \
  http://127.0.0.1:4010/api/inference/proxy
```

Expected: streamed SSE with `glm-5.3-flash`.

## 3. Local `pro` gateway

This is the OpenAI gateway. It only exposes an OpenAI-compatible surface and does not manage threads.

### Start

```sh
cd /Users/christopherdavid/work/pro
source /Users/christopherdavid/work/openagents.com/.env
export PRO_UPSTREAM_KEY=$OPENAI_API_KEY
cargo build -p pro-gateway   # if needed
/Users/christopherdavid/work/pro/target/debug/pro --bind 127.0.0.1:4100
```

### Verify the catalog

```sh
curl -s http://127.0.0.1:4100/api/v1/models | python3 -m json.tool
```

Expected: `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`.

### Important

The `pro` gateway does **not** have `POST /api/v1/threads` or `POST /api/v1/threads/{id}/grants`. `POST /api/v1/threads` returns `404`. It is a pure inference proxy. It is not a drop-in replacement for the Coder thread door.

## 4. CLI interactive test with `expect`

### `openagents coder` on Phoenix or `openagents-coder-api`

Example for `openagents-coder-api` on port 4010:

```sh
OPENAGENTS_API_URL=http://127.0.0.1:4010 \
OPENAGENTS_BASE_URL=http://127.0.0.1:4010/api/v1 \
openagents coder --lane flash
```

Example `expect` script (`test.exp`):

```tcl
set timeout 30
log_file -noappend /tmp/coder-test.log
stty columns 100 rows 30
spawn openagents coder --lane flash
sleep 2
send "hello\r"
expect {
    timeout { send \003; close; wait }
    eof { wait }
}
```

Run it:

```sh
expect test.exp
```

### What to check

- The `openagents-coder-api` or Phoenix log shows `POST /api/v1/threads`.
- The events table has a user message.
- The TUI prints the model response.

### Result in `rc.8`

The server paths above all stream, but the `expect` PTY cannot submit `Enter` in the TUI. The typed text appears on screen, yet no `POST /api/v1/threads/{id}/events` or proxy call happens. The `hello` prompt does not reach the server. This is a PTY input-handling issue, not a server issue.

## 5. `rc.8` `--dev` behavior

`rc.8` adds `--dev` which starts the `pro` binary on `127.0.0.1:4100` and points the session at it. In `rc.8`:

- `pro` starts.
- `GET /api/v1/models` works.
- `POST /api/v1/threads` fails with `404` because the `pro` gateway has no thread routes.

So `openagents coder --dev --lane pro` cannot complete a turn in `rc.8`.

## 6. Verdict checklist

| Check | Expected |
|-------|----------|
| `mix phx.server` starts without `PendingMigrationError` | Migrations applied |
| `GET /api/v1/models` | Returns `glm-5.3-flash` catalog |
| `POST /api/v1/threads` | Returns thread and grant |
| `POST /api/inference/proxy` | Streams SSE |
| `openagents-coder-api` catalog | Returns `glm-5.3-flash` |
| `openagents-coder-api` proxy | Streams SSE |
| `pro` catalog | Returns `gpt-5.6-sol` etc. |
| `pro` `POST /api/v1/threads` | `404` unless the route is added |
| `openagents coder` in `expect` | `Enter` does not submit in `rc.8` |

## 7. Notes for future agents

- Do not print tokens. Store them in a temp file and `source` it.
- The `pro` binary and `openagents-coder-api` are different. `pro` is OpenAI-only and has no thread state. `openagents-coder-api` is the local door for `flash`/`free`.
- `rc.8` `--dev` is wired to the `pro` binary, so it cannot exercise `flash` locally unless the CLI is changed to start `openagents-coder-api` or the `pro` gateway gains thread routes.
- For a reliable headless CLI test, find a way to send a true `Enter` key event. `expect`'s `send "\r"` and the Kitty `ESC[13u` sequence were not accepted by the `crossterm` TUI in `rc.8`.
