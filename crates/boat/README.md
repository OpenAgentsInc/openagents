# Boat Rust SDK

`boat` gives typed async access to all 69 operations of the
[Boat public API](https://docs.boat.dev/api/v1) (Boat was Ascii Box; the API
base is `https://boat.dev/api/v1`). It is a port of the private Coder repo's
`coder-box` (59 operations, 2026-09-05), regenerated from the pinned
`schema/boat-v1.yaml`. Plan: `docs/cloud/2026-10-02-boat-sdk-plan.md`.

## Create, run, stop

```rust,no_run
use boat::{Client, Nullable, WaitOptions, models::*};

# async fn example() -> boat::Result<()> {
// BOAT_API_KEY, else Secret Manager `boat-api-key`; BOAT_API_BASE overrides the base.
let client = Client::from_env().await?;
let created = client.create(&CreateParams {
    idempotency_key: Some("your-stable-job-id".into()),
    body: Some(CreateSandboxRequest {
        no_env: Some(true),
        ttl_seconds: Nullable::Value(600),
        ..Default::default()
    }),
    ..Default::default()
}).await?;
let id = created.sandbox.id;
client.wait_until_ready(&id, &WaitOptions::default()).await?;
let mut stream = client.exec_stream(&id, CommandRequest {
    command: "uname -a".into(),
    ..Default::default()
}).await?;
while let Some(frame) = stream.next().await? {
    if let boat::CommandFrame::Stdout(text) = frame {
        print!("{text}");
    }
}
client.stop(&StopParams { sandbox_id: id, ..Default::default() }).await?;
# Ok(())
# }
```

Each operation is a method named after its OpenAPI operation ID in snake case
and takes a borrowed `*Params` value: `body` is the JSON body, the other fields
are path, query and header values. `schema/operations.json` lists every
method, path, response type and retry class.

## Coverage

| Area | Operations |
| --- | --- |
| Account | `me`, `list_organizations`, `set_active_organization`, `limits`, data retention, deletion status |
| Keys and credentials | `api_keys`, `create_scoped_api_key`, `rotate_api_key`, `revoke_api_key`, `api_key_usage`, secrets, repositories |
| Environments | Create, list, update, delete, upgrade, variables, secret files, repositories |
| Sandboxes | Create, list, get, update, stop, resume, fork, share, delete, `usage` |
| Execution | `command`, `exec_stream`, `exec_detached`, `wait_command`, prompts, `steer`, `interrupt`, `conversations`, events |
| Access | Files, artifacts, desktop, SSH keys, hosted ports |
| Snapshots | History, latest, delete, named snapshots, trees, files, downloads |
| Webhooks | List, create, get, update, delete, rotate, `webhook::verify` |

## Credentials and output

- The key is an [`ApiKey`]: trimmed, never printed by `Debug` or `Display`, and
  not serializable. It is sent as a sensitive header, so it stays out of
  reqwest's debug output too.
- `ApiKey::resolve` reads `BOAT_API_KEY`, or Secret Manager `boat-api-key` in
  `openagentsgemini` through `gcloud` when the variable is unset.
- Every generated model's `Debug` prints only its type name. Errors carry the
  status, code, `requestId` and `Retry-After`, never the server's message,
  the request, or a URL.
- Redirects are refused. The base URL must be HTTPS (plain HTTP only on
  loopback, for tests).

## Organizations

`ClientBuilder::org` sets `X-Boat-Org` on the operations that accept it
(`limits`, `sandboxes`, `create`) unless the call's params set their own.

## Retries

`RetryPolicy` (default: 2 retries, 0.5 s doubling with jitter, `Retry-After`
honoured up to 20 s) applies only to:

- reads (`GET`), and
- `create` and `fork` when the params carry an `Idempotency-Key`; the same
  key and body are sent again.

Only 429 and 5xx are retried. Commands, prompts, stops and every other write
run once. A `502 boat_direct_failed` means the command may already be running:
check `Error::may_be_running()` and poll status instead of resending.

## Streaming and detached commands

`exec_stream` sends `stream: true` and yields `CommandFrame`s (`Started`,
`Stdout`, `Stderr`, then `Exit` or `Error`) from the NDJSON body, with a
bounded line buffer (1 MiB by default). `exec_detached` starts a background
process and `wait_command` polls it.

Boat has no endpoint for a detached command's output from an offset, for
signals, or for stdin, so `follow.rs` builds them from the public operations:

- `follow_command` yields the same `CommandFrame`s for a detached command:
  `Stdout`/`Stderr` in log order, then `Exit` (or `Error` with `"lost"`). It
  reads the process's log files from a byte `OutputCursor` with a short
  read-only command (base64, at most 128 KiB per stream per read), so output
  arrives exactly once and a split UTF-8 character waits for its next byte.
  Persist `cursor()` and call `follow_command_from` to pick up after a dropped
  connection or a restarted caller. Transient failures (transport, 429, 5xx,
  `boat_starting`) are ridden out up to five in a row.
- `kill_command(id, pid, Signal::Term)` signals the process and every
  descendant (`pgrep -P`), and says whether it was still running.
- `run_streaming(id, request, &options, sink)` streams a synchronous command
  into an async sink and returns the `Exit` or `Error` frame. The next chunk
  is read only after the sink has finished, so a slow consumer slows the
  stream rather than buffering it. A stream that closes early is
  `Error::Transport` (`may_be_running`).
- stdin: write the input with `write_bytes` and redirect it (`cmd < file`).
- Timeouts: `timeoutSeconds` (1-600) bounds a synchronous or streamed command
  on the server; `WaitOptions` bounds polling, following and `run_streaming`
  on the caller's side, and ending those never stops the remote command.

## Wire values

`Nullable<T>` keeps the three states of optional nullable fields: `Unset`
(omitted), `Null` (JSON null; for `ttlSeconds` this disables auto-archive) and
`Value`. Server enums are strings so new values still decode, and each struct
keeps unknown fields in `extra`.

## Regenerating

```sh
curl -fsSL https://docs.boat.dev/openapi/boat-v1.yaml -o crates/boat/schema/boat-v1.yaml
python3 crates/boat/schema/generate.py
```

Then update `SPEC_SHA256` in `src/lib.rs`; a test fails until it matches.
`generate.py` rewrites `src/models`, `src/api`, `schema/operations.json`,
`tests/contracts.rs` and `tests/published_examples.rs`.

## Tests

`cargo test -p boat` is offline:

- `tests/contracts.rs`: one generated request-encoding test per operation.
- `tests/fixtures.rs`, generated by `schema/fixtures.py` from
  `fixtures/spec/<operationId>.json` (all 69): each fixture's request goes
  through the SDK against a loopback server, and every success body (schema
  sample, each `oneOf` variant, each published example), every failure body
  (as an `Error::Api` with its envelope) and every capture in
  `fixtures/observed/` for that operation decodes.
- `fixtures/observed/`: the 96 redacted vendor captures from `coder-box`,
  ported to Boat's names; see `fixtures/README.md`.
- `fixtures/recorded/`: current read-only responses from `schema/capture.py`.
- The spec's published examples, transport, retry, streaming and follower
  tests.

The live test is ignored and also gated on an explicit cost acceptance. It
creates one `small` sandbox, runs `echo`, round-trips a file, follows and kills
detached commands, then stops and deletes it and fails unless `/usage` reads
under one cent and nothing is left running. The key comes only from
`BOAT_API_KEY`:

```sh
OA_BOAT_LIVE=I_ACCEPT_BOAT_COST cargo test -p boat --test live -- --ignored --nocapture
```

Build and run it on Boat, not locally: `scripts/boat-run.sh NAME -- cargo test -p boat`.
