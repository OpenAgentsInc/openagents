# Agent work on the website: Environments and Claude Code runs

2026-10-09, [#11162](https://github.com/OpenAgentsInc/openagents/issues/11162).

The website can do agent work for a signed-in person:

- **Environments** (`/environments`): pick a GitHub repository; a setup
  agent gets it building and testing on a machine; a clean build saves an
  image; a fresh machine checks it; the person saves the result.
- **Claude Code runs** from a chat (`/chat/{id}/claude`, and the composer's
  "Where it runs"): Claude Code works in a saved environment on a fresh
  machine, and its answer joins the chat.
- **Continue on a Cloud computer** (`/chat/{id}/continue`): a Coder chat
  whose computer is offline continues as a Claude Code run.

## Who can use it

`crates/openagents-web/src/agent_work.rs` decides it.

| Request | Gets |
| --- | --- |
| The local address (`127.0.0.1:PORT`) | Everything, as before |
| A public host, signed out | Sent to log in, then back |
| A public host, a site admin | Everything, for their own environments |
| A public host, an account in `OPENAGENTS_WEB_AGENT_ACCOUNTS` | The same (staging's smoke test account) |
| A public host, anyone else signed in | No link, and the site's not-found page |

A site admin is `admin: true` on the deployment's invite list
(`oa_auth::invite`, [GitHub sign-in](../auth/github.md)); the account
service reports it on `GET /v1/account`. Today that is only the owner.

Each environment records the account that made it. Lists, pages, posts,
streams, and runs reach only the person's own; another person's environment
answers not found. Environments made before they had owners show only on
the local address. Every post must come from the site itself
(`Sec-Fetch-Site: same-origin`, or a matching `Origin`), and the chat routes
also check their CSRF token.

## What it needs to run

The web process is only the controller. Machines never run in the web
container: setup, build, and check machines, and each Claude Code run, are
Boat machines (`crates/boat`), reached over HTTPS. Cloud Run cannot host
them (they need root, long-lived machines, and image capture).

| Need | Staging | Why |
| --- | --- | --- |
| Boat API key | Secret Manager `boat-api-key` as `BOAT_API_KEY` (runtime account `oa-vertex-inference` may read it) | Creates, drives, snapshots, and deletes the machines |
| A model for the setup agent | The gateway sidecar's `google/gemini-3.8-flash` on the house service key (`openagents/code` has no route for the house tenant on staging) (`$STACK_STATE/service.key`, the `stack` volume mounted read-only), via `model_api` in the studio config | No person is there to keep a Codex login fresh, so the Codex login path is for the local address only |
| A Claude credential for each run | The person's own key, saved in Settings > Claude (sealed with the BYO keyring) | No server-wide Claude key on staging |
| Durable records | `$WEB_STATE/environments` on the account-store NFS disk (`/state`) | Environments, their conversations, and run records outlive a revision |
| An always-on instance | min = max = 1, CPU not throttled | Setups and runs are driven by threads in the web process |

`deploy/staging/web.sh` writes the studio config and passes
`--environments` only when `BOAT_API_KEY` and the service key are both
there; if the studio can't open, the site starts without Environments and
the log says why. The Boat template is the newest ready
`oa-coder-runtime-*` snapshot, which carries Claude Code.

Limits on staging today:

- Public repositories only: setup machines get no GitHub credential.
- A deploy restarts the controller. Setups that were moving pick up again
  when it starts; a Claude Code run that was in flight stops being followed
  and its row stays Working.
- During a deploy, the old and the new instance both run for a minute or
  two over the same records.

## Smoke

`scripts/smoke/staging.sh --only gates,signed-in,environments` checks the
gate (signed out goes to log in; a signed-in person without agent work gets
not found and no link) and, as the fixed test account
`acct_dc7a799879686fc5` (key: Secret Manager
`openagents-web-1-staging-agent-smoke-key`, used only by the smoke), the
Environments pages, the repository list, a repository's branch step, and
that a post from another site is refused. `--environment-run OWNER/REPO`
also sets one up end to end on real Boat machines (up to 45 minutes).

The test account signs in with `POST /api/v1/sessions` and its key; the
alias forwards that route only on staging (`OPENAGENTS_WEB_API_OPERATOR_SIGNUP`).

## Production

On openagents.com since 2026-10-10 for the site admin (the owner) only;
`OPENAGENTS_WEB_AGENT_ACCOUNTS` is unset there. `deploy/production/web.sh`
carries the same launcher lines; `deploy/production/render.py` and
`scripts/deploy/web.sh promote` add `BOAT_API_KEY` (`boat-api-key`, read by
the runtime account `157437760789-compute`), `STACK_STATE`,
`ENVIRONMENTS_MODEL` and the read-only `/stack` mount when the live spec
lacks them, and refresh the launcher. `--production` smoke checks that a
signed-out visitor, and a forged session, are sent to log in from
`/environments` and a chat's Claude Code run.
