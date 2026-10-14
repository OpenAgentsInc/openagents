# Retiring the coder-serve sidecar

Issue #11614. Recorded 2026-10-14. The owner asked to remove `coder-serve`
from openagents.com and run everything on the new system.

Done 2026-10-14: the sidecar is gone (16:41 UTC), the pool groups are at 0,
and `coder-pg` is exported and stopped (23:27 UTC). See "Result" at the end.
The tables below describe the setup before the cut.

## What ran before the cut

Cloud Run service `coder` (us-central1, openagentsgemini) serves
openagents.com with three containers:

| Container | What it is | Size |
| --- | --- | --- |
| `web` | This repository's `crates/openagents-web`, port 8080. It serves the site, sign-in, chats, projects, settings, the phone and Coder sync APIs, OAuth, and the docs MCP. | 2 CPU, 1 GiB |
| `coder-serve` | The private `coder` repository's whole server (`bins/coder-serve`), unchanged, on 8081. `web` sends it every path it doesn't own (`OPENAGENTS_WEB_UPSTREAM`). It has its own database (`coder-pg`) and a scaler that sizes the `coder-pool*` VM groups. | 4 CPU, 8 GiB |
| `gateway` | `openagents-stack`, port 8791: the account service (accounts, sessions, keys) and the inference gateway (Vertex first). | 1 CPU, 512 MiB |

`coder-serve` is not built on `crates/coder-sdk`. It links its own private
crates (`coder-auth`, `coder-box-control`, `coder-runner`, `coder-contract`,
and others) and duplicates most of what `web` and `gateway` now own.

## Measured traffic

The sources are coder-serve's own request log (`coder::security answered`,
2026-10-07 to 2026-10-14) and the web's routing rules run over 12 hours of
Cloud Run request logs (about 192,000 openagents.com requests).

- 4.4% of openagents.com requests reach coder-serve. Almost all of them
  answer 404: scanners (`/.env`, `/wp-*`, `*.php`) and old apps calling
  endpoints that were already removed (`/api/mobile/signin-debug`,
  `/api/nodes/ping`, `/api/mdk`, `/api/agents/register`,
  `/api/public/product-promises`).
- Live paths, over 7 days:

| Path | Requests | Who | Fate |
| --- | --- | --- | --- |
| `POST /api/box-host/v1/heartbeat`, `/calls`, `/jobs` | 83,700 | The two idle box VMs (`coder-pool-w2v4`, `coder-box-pool-8b1t`), on the `run.app` host. Every jobs and calls poll answered 204 (no work). | Dead. They get 403 after the cut. Scale the groups to 0. |
| `POST /computers/seen` | 37,000 | The old Coder Terminal (`coder-terminal/0.5.0`) in earn mode. Earn was removed 2026-09-29. | Dead: 404. |
| `POST /mcp` | 844 OK | The owner's Claude Code, with the OpenAgents connector at `https://openagents.com/mcp`. Last success 2026-10-12 22:00. | Ported: the site answers `/mcp` itself. |
| `GET /v1/responses` (WebSocket) | 171 | A steady client with no user agent, about 26 a day. | Moved to the gateway's `/v1/responses`. |
| `POST /v1/responses` (lane words, run header) | 11 a day | Scripts (`Python-urllib`, `curl`). | Moved to the gateway. Lane words and coder-serve-only keys now get the gateway's answer. |
| `GET /v1/models`, `/api/v1/models` on non-site hosts | 58 | Tools and tests. | Gateway. |
| `/releases/*`, `/install-terminal.sh` | 7 | The old Coder Terminal installer. It was already unlinked, and the owner removed it 2026-09-29. | Dead: 404. |
| `/u/{login}` | 6 | Profiles. | Already drawn by `web`'s own router. |

Nothing about sign-in (`/auth/*`, `/login`, `/device`, `/oauth/*`),
payouts or wallet (`/api/flow/*` and `/api/stats` go to the pay host), Pylon, or Coder
downloads (`/download`, `/cli/install.*`) goes through coder-serve: `web`
owns all of them.

## Duplicates (both systems have one)

| Feature | New system | coder-serve copy |
| --- | --- | --- |
| Accounts, GitHub sign-in, sessions | `gateway` account service + `web` `/auth`, `/login`, `/device` | `/login`, `/auth/github`, `/auth/agent`, `/email`, `/invite` |
| API keys, tokens | `gateway` `/v1/workspaces/*/keys`, `web` sign-in keys | `/v1/token`, `/v1/grant`, `/.well-known/coder-issuer` |
| Model door `/v1/responses`, `/v1/models` | `gateway` (Vertex first) | Vercel AI Gateway door (#11602) |
| Runs, cloud computers | `web` own runs, work runs, Environments on oa-boat | `/run`, `/boxes`, `/jobs`, `/api/box-host`, coder-pool VMs |
| MCP | `web` `/mcp/docs`, now `/mcp` | `/mcp` (run, thread, autopilot, forum tools) |
| Chats and sync | `web` `/v1/threads`, `/coder/*` sync | `/threads`, `CODER_CHAT_SYNC` |
| Computers | `web` `/v1/computers/*` | `/computers/pair`, `/computers/seen` |
| Earn | removed | `/earn`, `/v1/earn` |
| Releases | `web` `/download`, `/cli/install.*` | `/releases/*` (old Terminal) |
| Calendar booking | none (not used) | `/calendar/booking` |
| Billing | `gateway` balances and top-ups, pay host | `/v1/credit`, Stripe webhooks |
| Docs, forum, gym, traces, QA | `web` `/docs`, `/v1/traces`; forum, gym and QA removed | all of them |

## The cut

1. Web (this change): with no upstream configured,
   - the gateway's public `/v1` routes (the `/api/v1` alias list,
     `api_alias::forwards`) answer at `/v1/...` too, cookies removed;
   - the site serves the keyed `/mcp` itself (`docs_mcp.rs`): the docs
     tools plus `whoami`, for a `sess_` bearer the account service accepts.
     Without one it answers 401 with the OAuth challenge, so the connector
     signs in again through `/oauth/authorize`;
   - every other path the site doesn't draw answers 404 and writes
     `retired: METHOD /first-segment[/*]` to the log (never a body, token,
     or id).
2. Deploy: `scripts/deploy/web.sh promote --retire-serve DIGEST` copies the
   serving spec without the `coder-serve` container, without the web's
   `OPENAGENTS_WEB_UPSTREAM`, without the `container-dependencies` entry,
   and without `coder-pg` in the Cloud SQL list. `deploy/production/render.py`
   renders the same. The candidate is smoked at the `new` tag, then
   `shift`. Record the rollback revision in the issue.
3. Watch: count the `retired:` lines for a day:
   `gcloud logging read 'resource.labels.service_name="coder" AND textPayload:"retired:"'`.
   A path that shows real use gets a port or a redirect, not the sidecar.
4. Owner, after a quiet day: scale the managed instance groups
   `coder-pool`, `coder-box-pool`, and `coder-pool-spot` to 0 (two
   c3-standard-8 VMs that only send heartbeats), export `coder-pg`, then
   stop it.

Rollback: `scripts/deploy/web.sh rollback REVISION` to the revision that
served before the cut. It still has the sidecar.

## Result (2026-10-14)

- 16:41 UTC: `coder-web-578e2eec43-20261014164037` (commit `7f2a89a14e`)
  serves without the sidecar. Rollback revision with the sidecar:
  `coder-web-627d689d17-20261014163441`.
- `coder-pool`, `coder-box-pool` and `coder-pool-spot` (us-central1-b) are
  at size 0. They are kept, not deleted; `gcloud compute instance-groups
  managed resize NAME --size 1 --zone us-central1-b` brings one back.
- `retired:` lines, 16:41 to 23:20 UTC (6.6 h): 3,164. By who sent them:
  - 1,538 `POST /computers/seen`: one old Coder Terminal 0.5.0 in earn mode,
    on the owner's network. Earn was removed 2026-09-29; nothing to port.
    Quitting or updating that Terminal stops them.
  - About 1,600 scanners: `/wp-*`, `*.php`, `/.git`, `/cgi-bin`,
    `/.well-known/*` probes (agentgrade, AgentReadyCheck,
    isitagentready), `/api/nodes/ping` from search bots.
  - Old clients that got 404 before the cut too: `POST /api/mdk` (an MDK
    webhook pinger), `/api/pylons/*` (an old Bun Pylon),
    `POST /api/agents/register`, and the smoke's own negative checks on
    `/api/v1/sessions/*`.
  - No real user path. The keyed `/mcp` is the site's (401 with the OAuth
    challenge, then `/.well-known/oauth-protected-resource/mcp` and
    `/.well-known/oauth-authorization-server`); `/v1/responses` and
    `/v1/models` answer from the gateway.
- `coder-pg` had no writes after the cut (its last autovacuum of
  `coder.public.computers` was at 16:39 UTC), and no Cloud Run service lists
  it. Exported at 23:25 UTC with `gcloud sql export sql`:
  - `gs://openagentsgemini-coder-pg-export/coder-pg-final-20261014.sql.gz`,
    database `coder`, 140,624,451 bytes, gzip intact, 97 tables;
  - `gs://openagentsgemini-coder-pg-export/coder-pg-final-20261014-postgres-db.sql.gz`,
    database `postgres` (454 bytes, empty).
  The bucket is private (uniform access, public access prevention); only the
  Cloud SQL service agent was added, as object admin.
- 23:27 UTC: `coder-pg` stopped (`--activation-policy=NEVER`), not deleted.

Restart `coder-pg` (needed before any rollback to the sidecar revision):

```sh
gcloud sql instances patch coder-pg --activation-policy=ALWAYS --project openagentsgemini
scripts/deploy/web.sh rollback coder-web-627d689d17-20261014163441
```

Restore into a new instance instead: `gcloud sql import sql INSTANCE
gs://openagentsgemini-coder-pg-export/coder-pg-final-20261014.sql.gz
--database=coder --project openagentsgemini`.
