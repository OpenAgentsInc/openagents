# OpenAgents website port

Status of moving the openagents.com website from the private `coder`
repository (`bins/coder-serve`) into this repository's
[`crates/openagents-web`](../../crates/openagents-web/README.md). Recorded
2026-09-29.

## Live (2026-10-01)

openagents.com serves this site since 2026-10-02 02:20 UTC (#10128): Cloud
Run service `coder` (us-central1, openagentsgemini), revision
`coder-web-79692f5e9a` since 2026-10-02 06:55 UTC (the complete user guide
at `/docs`; the one before it, `coder-web-dcc80c9096`, the move to
`/download`, is the rollback), two containers. `web` is this crate's image
(`openagents/openagents-web:79692f5e9a`, built by `crates/openagents-web/cloudbuild.yaml`)
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
  a tag, check the tag URL, then move traffic. The export writes
  `CODER_CHAT_SYNC`'s value as a bare `on`, which `replace` refuses as a
  boolean; quote it (`'on'`). Use the tag `new`: the `web` container
  already lists `new---coder-ezxz4mgdsq-uc.a.run.app` as a `--public-host`,
  so that tag URL serves the site, while any other tag's host is proxied
  whole to `coder-serve`. With the image built from GitHub instead of a
  local upload, pass the full commit to `--git-source-revision` and
  `--service-account` (the org policy requires one). On 2026-10-02 the
  automation account was refused `iam.serviceAccounts.actAs` on the
  runtime account `157437760789-compute@developer.gserviceaccount.com`,
  though that account's policy lists it as `roles/iam.serviceAccountUser`,
  so the revision was applied as `chris@openagents.com`. The same held for
  `coder-web-79692f5e9a`: Cloud Build ran as the automation account
  (`gcloud builds submit https://github.com/OpenAgentsInc/openagents
  --git-source-revision=SHA --service-account=…oa-mvp-automation…`), and
  `replace` was refused `actAs` again and applied as `chris@`.
- Roll back to the old site: `gcloud run services update-traffic coder
  --region us-central1 --project openagentsgemini --to-revisions coder-00168-smb=100`.

The port reimplements the pages. It copies the published content (the terms
and the privacy policy) and no backend code,
prompts, endpoints, or secrets. The site is now drawn in four intensities of
white on near-black instead of four intensities of amber.

Built, not yet live (2026-10-02 23:30 UTC): image
`openagents/openagents-web:3a46b3c415` (`/live` and `/stats`, #10196) was
built by Cloud Build as the automation account, but `services replace` of
revision `coder-web-3a46b3c415` (no traffic, tag `new`) was refused
`iam.serviceAccounts.actAs` on the runtime account again, although that
account's policy still lists the automation account as
`roles/iam.serviceAccountUser` and `testIamPermissions` grants it only
`get` and `setIamPolicy` there; something above the account's own policy
withholds `actAs`. Apply it as `chris@` with the steps above. No pay host
is wired yet (`OPENAGENTS_WEB_PAY_HOST` is unset), so after the deploy
`/api/flow/*` and `/api/stats` answer `503`, `/live` says the stream is
unreachable, and `/stats` says the statistics are unreachable.

`/live` (#10197) is on `main` but not deployed: it reads
`/api/flow/snapshot` and `/api/flow/stream` on this origin, which this
server proxies to the pay host once #10195 lands. Until then it says the
flow stream is unreachable. Deploy it with the next site build after
#10195 is live.

## Ported and complete

These pages need nothing more than this repository and the public buckets:

- `/`, the homepage: what OpenAgents is, one `[ Download OpenAgents ]` link
  to `/download`, and the **Ask OpenAgents** terminal. The Grid's
  screenshot moved to its own guide, `/docs/the-grid`, on 2026-10-01 so the
  homepage leads with the terminal. The old terminal and its `/ask` box were removed on 2026-09-29 and
  brought back on 2026-10-01 at the owner's direction
  ([#10106](https://github.com/OpenAgentsInc/openagents/issues/10106)), now
  on the white theme and tied to the same OpenAgents chat worker the apps
  use, as the `web` surface: answers and knowledge about OpenAgents only,
  pointing to `/download` for anything the apps do; never Coder or a
  computer. `help`, `download` (`install` is its alias), `docs`, and
  `clear` are its commands. A
  deployment with several instances sets `OPENAGENTS_WEB_ASK_SALT` (64 hex
  characters, a secret) so a visitor keeps one signing key; it needs no
  model key, since the chat worker holds its own.
- `/download`: the one download page (`src/pages/download.rs`): the
  notarized OpenAgents for Mac `.dmg` in `openagentsgemini-oa-updates`,
  OpenAgents Terminal's install commands, and one link to build everything
  else from source. It was `/install` until 2026-10-01; `/install` and
  `/desktop` redirect here permanently (`308`), and the guide
  `/docs/install` to `/docs/download`.
- `/terms` and `/privacy`: the published text, last updated 2026-09-03,
  unchanged and compiled in.
- `/connect`, `/.well-known/apple-app-site-association`, and
  `/.well-known/assetlinks.json`: the pairing link's landing page and the
  app-association files, as the `coder` repository serves them.
- `/docs` and `/docs/{slug}`: the user guide
  (`crates/openagents-web/content/docs/`), thirty short pages in nine
  sections (`SECTIONS` in `src/pages/content.rs`): getting started, the
  apps, chat, Coder, computers, the four plugin guides
  ([#10088](https://github.com/OpenAgentsInc/openagents/issues/10088)),
  the Gym, Verse, wallet, and decks, reference, and the FAQ and glossary.
  Expanded on 2026-10-02 at the owner's direction; `/docs/help` redirects
  to `/docs/troubleshooting`.
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
6. Update `/download` for each desktop release (`MAC_VERSION` and `MAC_DMG`).
7. When this server serves openagents.com, update the `INVARIANTS.md` row
   for the connect link, which names `coder-serve` as the server of
   `/connect`.
8. Cut DNS over from the `coder` deployment, then retire its site routes.
