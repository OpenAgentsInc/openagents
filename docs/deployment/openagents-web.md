# OpenAgents website port

Status of moving the openagents.com website from the private `coder`
repository (`bins/coder-serve`) into this repository's
[`crates/openagents-web`](../../crates/openagents-web/README.md). Recorded
2026-09-29.

## Live (2026-10-01)

openagents.com serves this site since 2026-10-02 02:20 UTC (#10128): Cloud
Run service `coder` (us-central1, openagentsgemini), revision
`coder-web-a0c19b1829-b`, two containers. `web` is this crate's image
(`openagents/openagents-web:a0c19b1829`, built by `crates/openagents-web/cloudbuild.yaml`)
on port 8080 with `--public-host openagents.com`; it serves its own pages
and passes every other path (APIs, `/v1`, `/mcp`, `/auth`, `/computers/seen`,
`/releases`, …) to `coder-serve`, the previous production image and config
unchanged, as a sidecar on port 8081 (`OPENAGENTS_WEB_UPSTREAM`), with the
original Host so sign-in and billing links stay on openagents.com. Removed
sections stay 404. `OPENAGENTS_WEB_ASK_SALT` is the secret
`openagents-web-ask-salt`.

- Deploy a new site build: build the image with Cloud Build, then replace
  the `web` container's image in a copy of the live revision spec
  (`gcloud run services describe coder --format export`) and apply it with
  `gcloud run services replace` under a new revision name with no traffic and
  a tag, check the tag URL, then move traffic.
- Roll back to the old site: `gcloud run services update-traffic coder
  --region us-central1 --project openagentsgemini --to-revisions coder-00168-smb=100`.

The port reimplements the pages. It copies the published content (the terms
and the privacy policy) and no backend code,
prompts, endpoints, or secrets. The site is now drawn in four intensities of
white on near-black instead of four intensities of amber.

## Ported and complete

These pages need nothing more than this repository and the public buckets:

- `/`, the homepage: what OpenAgents is, one `[ Install OpenAgents ]` link
  to `/install`, and the **Ask OpenAgents** terminal. The Grid's
  screenshot moved to its own guide, `/docs/the-grid`, on 2026-10-01 so the
  homepage leads with the terminal. The old terminal and its `/ask` box were removed on 2026-09-29 and
  brought back on 2026-10-01 at the owner's direction
  ([#10106](https://github.com/OpenAgentsInc/openagents/issues/10106)), now
  on the white theme and tied to the same OpenAgents chat worker the apps
  use, as the `web` surface: answers and knowledge about OpenAgents only,
  pointing to `/install` for anything the apps do; never Coder or a
  computer. `help`, `install`, `docs`, and `clear` are its commands. A
  deployment with several instances sets `OPENAGENTS_WEB_ASK_SALT` (64 hex
  characters, a secret) so a visitor keeps one signing key; it needs no
  model key, since the chat worker holds its own.
- `/install`: new. The one install page for everything being launched, in
  order: the notarized OpenAgents for Mac `.dmg` in
  `openagentsgemini-oa-updates`, the iPhone app on TestFlight, pairing by
  the Mac's QR code, and signing in to Codex or Claude Code on the Mac so
  the phone can run Coder. Android, Linux, and Windows are named as not yet
  available. `/desktop` redirects here permanently.
- `/terms` and `/privacy`: the published text, last updated 2026-09-03,
  unchanged and compiled in.
- `/connect`, `/.well-known/apple-app-site-association`, and
  `/.well-known/assetlinks.json`: the pairing link's landing page and the
  app-association files, as the `coder` repository serves them.
- `/docs` and `/docs/{slug}`: new on 2026-09-29, short guides to the apps
  we launch (`crates/openagents-web/content/docs/`), and on 2026-10-01
  four guides to plugins: Plugins, Write a plugin, Test a plugin, and
  Publish and share ([#10088](https://github.com/OpenAgentsInc/openagents/issues/10088)).
  Committed, not deployed.
- `/app`: the local task browser, loopback only.

## Ported, waiting on a production backend

These pages render on a development server and say that their data needs
the production backend. Each reads through the `Backend` trait
(`crates/openagents-web/src/backend.rs`), which has no production
implementation yet:

| Page | Backend method | Production source |
| --- | --- | --- |
| `/u/{login}` | `profile` | The account store. |
| Homepage credit line | `new_account_credit_cents` | Service configuration. |

## Removed

The Forum (`/forum`), Gym (`/gym`, `/gym/results/...`), Traces (`/traces`,
`/trace/{key}`), Earn (`/earn`), Weights (`/weights`), and QA (`/qa`)
sections were ported and then removed on 2026-09-29 at the owner's
direction: the site serves none of them and links none of them.

Also removed on 2026-09-29 at the owner's direction, because the old Coder
Terminal product (installed from the private `coder` repository's bucket
`openagentsgemini-cli-releases`) is not connected to OpenAgents: the
`/releases/{name}` release proxy, the `/install-terminal.sh` and
`/install-terminal.ps1` redirects, the homepage's install commands, and the
old Docs (`/doc*`) and Blog (`/blog*`) sections, whose every document was
about Coder Terminal. All of them answer `404`. The old site's `/docs`
pages were about Coder Terminal too; this server's `/docs` is new and
serves different guides at that path.

## Not ported

These need accounts, sessions, or payments, or are internal:

- Sign-in and accounts: GitHub OAuth, sessions, `/login`, `/logout`,
  `/auth/*`, invites, the waitlist, `/settings`, agent approval, and
  computer pairing codes.
- Billing: Stripe checkout and webhooks, credit, and Pro.
- The run console: `/run/{id}`, `/ws`, `/message`, cancel, rerun, archive,
  and `/trajectories`.
- APIs: `/v1/*`, `/api/*`, MCP, the decision door, token and grant minting,
  earn receipts, trace upload, and QA uploads.
- Avatars (`/u/{login}/avatar`), calendar booking, `/map`, `/components`,
  `/showcase`, deployments, and page-view analytics.

## Before deploy

1. Implement `Backend` against production data (profiles, homepage
   answers, and the credit line), or launch without them.
2. Package and host the binary (for example on Cloud Run) with
   `--listen 0.0.0.0:PORT --public-host openagents.com`, behind TLS.
3. Serve the association files at `openagents.com` with no redirect, and
   check that iOS and Android still verify `/connect`.
4. Keep the URLs that must not move the same as the `coder` repository
   serves them: the pairing link (`/connect`) and the legal pages.
5. Decide the old Coder Terminal's fate. This server does not serve
   `/releases/*`, `/install-terminal.sh`, `/install-terminal.ps1`, the old
   `/docs` pages, or `/blog`. Once DNS moves, the installed Coder
   Terminal's self-update (which reads `/releases/`) and its published
   install command (`curl -fsSL https://openagents.com/releases/install-terminal.sh | sh`,
   and the PowerShell form) stop working, and old docs and blog links
   answer `404`. That is an owner decision: accept the break, or restore a
   redirect to the bucket before cutover.
6. Update `/install` for each desktop release; it links version 1.0.0.
7. When this server serves openagents.com, update the `INVARIANTS.md` row
   for the connect link, which names `coder-serve` as the server of
   `/connect`.
8. Cut DNS over from the `coder` deployment, then retire its site routes.
