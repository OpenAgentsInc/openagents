# boat: the Coax SDK for boat sandboxes

Claude Code dev boxes run on hosted boat ([#13474](https://github.com/OpenAgentsInc/openagents/issues/13474);
[the audit](../../../docs/audits/2026-11-13-boat-claude-devboxes.md), [the
runbook](../../../docs/cloud/boat-devbox.md)). This folder is how Coax talks to
boat's v1 HTTP API: no CLI wrapper, no generated client.

| File | What |
| --- | --- |
| `wire.coax` | The one codec (`proto/registry.json`, `boat.v1`): every request body and path, and readers from boat's answers into our records (`BtSandbox`, `BtLimits`, `BtCommand`, `BtFrame`, `BtSaved`, `BtSsh`, `BtUsage`, `BtEnv`) and a typed `BtError` |
| `client.coax` | `BoatClient` on the gateway's curl transport (`coax/lib/gateway/transport.coax`): one synchronous call, or an NDJSON stream read line by line without blocking. Holds the hosted guard, and a fake for checks |
| `boat.coax` | The calls: limits; create, get, list, stop, resume, set the auto-stop, delete, usage, wait until up; commands synchronous, streamed, and detached with attach, status and kill; files; saved snapshots; SSH keys; environments |
| `policy.coax` | `#authority`, zero surviving mutants: the sizes we use, 2 boxes at once, auto-stop 15 minutes to 24 hours (12 by default, never off), $5.00 of box time a UTC day, and which calls may be retried and when |
| `ssh.coax` | The `~/.ssh/config` block (`Host boat-ID`) and the strict known-hosts line, rewritten from boat's `sshkey` answer after every resume |
| `checks.coax` | Every call and error against canned answers. No check sends anything |

```coax
#load "coax/lib/boat/boat.coax";

bb: [512] u8; c := bt_client_env(bb[..]);     // BOAT_API_BASE, BOAT_HOSTED=1, key in BOAT_API_KEY
out: [262144] u8; ab: [65536] u8; ar: [1] ClArena; ar[0] = ClArena.{buf = ab[..]};
r := bt_resume(c, "bx_fwh3xnws", BtResume.{ttl_s = 43200}, out[..], ar[..]);
if bt_failed(r.error) { /* r.error.kind: rate_limited, stopping, payment_required ... */ }
w := bt_wait_up(c, "bx_fwh3xnws", 120, out[..], ar[..]);
```

## Rules it keeps

- **The key never shows.** curl reads it from `BOAT_API_KEY` (`--variable`)
  and writes the header itself. A key from a file goes into that variable in
  curl's environment only. No error keeps boat's `message`; it keeps the
  status, code, request id and wait.
- **Hosted boat is opt-in.** A base on `boat.dev` is refused unless
  `BOAT_HOSTED=1`. Plain `http` only on loopback. curl follows no redirect.
- **Secrets travel in a named environment, never in `env`.** `BtCreate` has
  no `env` field: a saved snapshot copies a box's `env` to every box made
  from it. A variable's value goes in a request body only, and the SDK never
  reads an environment's `envContents`.
- **Retry only what is safe.** `bt_call` sends a call again only when it is
  safe (a GET, a create with an `Idempotency-Key`, or one boat says never
  ran) and boat says a retry can help, as `bp_retry_wait` decides: at most 4
  times, after `Retry-After` (60 s at most) or 1, 2, 4, 8 s. Never a
  command, a stop or a resume. A `502 sandbox_direct_failed` is
  `may_be_running` unless boat says the command never ran (`restoring`).
- **Two facts to plan around.** A stop answers while the box is still
  `archiving`; a resume then gets `409 sandbox_stopping` (kind `stopping`,
  retryable) and then the same machine. A synchronous command is killed at
  its timeout (600 s at most): longer work runs detached and is followed with
  `bt_attach` from a stored `BtCursor`, which reconnects every 10 minutes
  with every byte once.

## From a terminal

`coax/web/tool.sh boat` (`coax/web/boat.coax`) is the SDK and the policy for
people: `ls`, `up`, `resume`, `stop`, `ssh`, `cost`. Every cap is printed by
`ls` and `cost`. The key comes from `BOAT_API_KEY`, or
`~/.openagents/boat/api-key` (mode 0600).

## Switching to our own service

`BOAT_API_BASE` points the whole SDK at any Boat-shaped service (stage 6, the
exit plan). Nothing else changes.

## Checks

```sh
coax/bin/coax test --library coax/lib/boat/boat.coax
coax/bin/coax test coax/lib/boat/policy.coax && coax/bin/coax mutate coax/lib/boat/policy.coax
```

The fixtures in `checks.coax` are boat's own bodies where the old Rust SDK
captured them (`git show 853417f290:crates/boat/fixtures/observed/`), and
bodies shaped from boat's OpenAPI file with stage 0's values otherwise. A
live check runs only by hand, with a scoped key.
