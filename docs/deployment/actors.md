# The actor runtime in the website (#11253)

`crates/actors` runs inside `openagents-web` (step 0 of the
[revised consumer plan](https://github.com/OpenAgentsInc/openagents/issues/11253#issuecomment-6103315086)),
and Mac jobs (#11223) are its first consumer (step 1), behind a flag.

## What runs where

| Part | Where | Notes |
| --- | --- | --- |
| Store | The account database: Cloud SQL `openagents-staging-pg` / `openagents-production-pg` (PostgreSQL 18), database `openagents`, schema `actor` | Reached as the gateway reaches it, through the Cloud SQL connector's socket (`run.googleapis.com/cloudsql-instances`) with the login in `openagents-<env>-pg-dsn`. The web container gets it as `OPENAGENTS_WEB_ACTORS_DATABASE_URL`. The crate's own digest-checked migration runs at start under its advisory lock. Pool: 4 connections plus one listener per process |
| Workers | Every `openagents-web` process (`actors::Runtime`) | Inboxes, alarms, lease expiry. Any number of processes may run them on one database (a deploy briefly runs two) |
| Routes | `/v1/w/{workspace}/…`, `/v1/actors/contract.json` | `actors::http::router` behind [`actors_host::Auth`](../../crates/openagents-web/src/actors_host.rs) |
| Mac jobs | `OPENAGENTS_WEB_MAC_JOBS_ACTORS=1` on the web container | Off (`0` or unset): the chat-store jobs, as before |

If the database can't be reached or migrated at start, the site logs
`actors: not started` and runs without actors (Mac jobs fall back to the
chat store).

## Authority

- **Who.** An app's own token (`Authorization: Bearer sess_…`), checked with
  the account service, or the browser's session cookie for reads only
  (cookies are removed from every other method, so no page can be made to act
  for its visitor). The principal is `account:<id>`.
- **Where.** The workspace in the URL must have the account as an active
  member, read from `workspace.memberships` (the account store's own table):
  `owner` and `admin` are the actor role `owner`, `member` is `member`.
  Mac jobs live in the account's own (personal) workspace.
- **Never from HTTP.** Service and administrator roles. The website's own
  pages act as the account with service authority (`Host::host_caller`),
  which is how only the job page and the phone's board can answer an
  approval. Operators use `actors-admin`.
- **Executors.** A linked Mac adds `X-OpenAgents-Computer: <name>` to its own
  token; its grant is exactly the queue `mac-jobs` and that Mac's target, one
  claim at a time, for an hour (renewed with each request).
- **Revalidation.** A queued message or alarm runs under its saved caller
  only while that account is still an active member of the workspace.

## Mac jobs through actors

One `mac.job` actor per run ([`crates/mac-jobs/src/actor.rs`](../../crates/mac-jobs/src/actor.rs)):

1. `POST /v1/mac-jobs` creates it with `submit@1` (host authority). The job id
   comes from the request's `Idempotency-Key` (`openagents mac run` sends one
   and retries with it), so a retry is the same job, never a second run.
2. The job offers one work item on `mac-jobs` for its Mac. `openagents mac
   serve` learns where to claim from its capability report's answer
   (`actors: {workspace, queue, target}`), then claims with a 25 s long poll
   through `actors::net::Client`.
3. The Mac reports with `report@1`, fenced by its claim (only that claim, at
   that epoch, before its 60 s lease ends; each report renews it), and a
   thread heartbeats every 15 s while a step is quiet. Files go up through the
   old part route with `item` and `epoch`, and `artifact@1` records each part.
   It ends the claim with the queue's finish (`WorkDone`), or, cancelled, with
   a release.
4. An upload asks; the question is bound to the claim's epoch. `approve@1` and
   `deny@1` need host authority: the web form and the phone's Approve/Deny.
   The answer is handed to that epoch's claim once. A denial sticks.
5. A Mac that stops answering loses the claim when the lease ends. A test or
   build goes back to waiting and runs again on the next claim, at a new
   epoch; the old epoch is refused everywhere. An upload becomes `uncertain`
   and holds the Mac's slot until an operator records what happened.
6. `/settings/mac-jobs` and the phone read `GET /v1/mac-jobs/events`, an
   event stream that fires when any of the account's jobs changes, and
   reload or read again on it.

Jobs made before the switch stay in the chat store and finish there; every
route reads the actor first, then the old store.

## Operators

`scripts/actors/admin.sh staging|production COMMAND …` runs `actors-admin`
through the Cloud SQL proxy as the automation account; the connection string
goes from Secret Manager into that process's environment only.

```sh
cargo build -p actors --bin actors-admin
export ACTORS_ACCOUNT_ID=acct_…            # the job's account
scripts/actors/admin.sh staging list WORKSPACE mac.job
scripts/actors/admin.sh staging inspect WORKSPACE mac.job mjob…
scripts/actors/admin.sh staging history WORKSPACE mac.job mjob…
# An uncertain upload: record what App Store Connect shows, or allow a retry.
echo '{"expected_epoch":N,"retry":false,"outcome":{"done":{"summary":"Uploaded (checked)."}}}' |
  scripts/actors/admin.sh staging resolve-work WORKSPACE UID ITEM
```

`inspect` gives the `uid`; the work item and epoch are in the job's state
(`item`, `epoch`; the expiry bumps the epoch by one).

## Flipping the flag

```sh
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config gcloud run services update \
  openagents-web-1-staging --region us-central1 --project openagentsgemini \
  --container web --update-env-vars OPENAGENTS_WEB_MAC_JOBS_ACTORS=1   # or 0
```

Production is the service `coder` (same flag on its `web` container); a
`scripts/deploy/web.sh promote` keeps whatever the live spec has, and adds the
database variable when the spec lacks it. Staging's `render.py` sets the flag
(`MAC_JOBS_ACTORS`). Turning it off sends new jobs to the chat store; actor
jobs already made stay readable while the database is configured.

## Log

**2026-10-11, staging** (`openagents-web-1-staging`, `f9db8bca3d`). Actor
runtime started on `openagents-staging-pg`, migration applied; full smoke 92
passed, 0 failed. Flag on. With this Mac as the linked computer (`openagents
mac serve`, the smoke test account's session) and `openagents mac run`:

- `ios-release-gate` on `main` ran end to end through the `mac.job` actor:
  1 of 1 tests passed, 55 files (97 MB) back with `--out`.
- Kill and recovery: the Mac was killed (SIGKILL) right after checkout; the
  lease ended, the claim was fenced, the job went back to waiting ("The Mac
  stopped answering…"), and the restarted Mac took it again at epoch 3.
- Upload: `ios-testflight --validate-only` stopped at the question; approving
  through the generic action API was refused (`forbidden`); Deny from the
  phone route was recorded, the Mac stopped ("You denied the upload, so
  nothing was sent."), and a later Approve was refused (`not_asking`).
- A Mac killed while an upload waited: uncertain within ~70 s; `actors-admin
  resolve-work` refused the stale epoch and recorded the outcome at the
  current one; `history` shows each transition and the operator's
  `work.resolve`.
- Retries: `mac run` sends one `Idempotency-Key`; the same key is the same job
  (tests: replay, no second work item, a different body refused).
- Live: `/settings/mac-jobs` and its job page carry the event stream and
  reload on change; `GET /v1/mac-jobs/events` answers app tokens (the phone)
  and cookies; the generic `/v1/w/…/events` stream answers a cookie read.

Found and fixed on the way: the actor routes were nested, so the outer path
parameter reached them (now merged, with a router test); a file part's work
item id was cut to 64 characters by the computer-name encoder; heartbeats
changed versions and reloaded live pages (now views are watched); and
`crates/openagents-mobile/Cargo.lock` lacked pylon's `base64` and `sha2`, which
failed the gate's `--locked` iOS build on main.

**2026-10-11, production** (`coder`). `promote` of the staging image
(`sha256:d63bc625…`): the actor runtime started on `openagents-production-pg`,
flag off; shifted, `--production` smoke 59 passed. Then revision
`coder-web-d63bc62598-macjobs-030313` with `OPENAGENTS_WEB_MAC_JOBS_ACTORS=1`,
shifted, smoke 59 passed; `/v1/actors/contract.json` lists `mac.job`. No
production Mac serves yet (NEEDS_OWNER: `coder login`, install from main,
`openagents mac serve`), so no production job has run.
