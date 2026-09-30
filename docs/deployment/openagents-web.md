# OpenAgents website port

Status of moving the openagents.com website from the private `coder`
repository (`bins/coder-serve`) into this repository's
[`crates/openagents-web`](../../crates/openagents-web/README.md). Recorded
2026-09-29. openagents.com still deploys from the `coder` repository; nothing
here is deployed.

The port reimplements the pages. It copies the published content (the terms,
the privacy policy, the public docs, and the blog post) and no backend code,
prompts, endpoints, or secrets. The site is now drawn in four intensities of
white on near-black instead of four intensities of amber.

## Ported and complete

These pages need nothing more than this repository and the public buckets:

- `/`, the homepage: welcome card, install commands, desktop link, and an
  ask box that works without a script (`/ask`).
- `/terms` and `/privacy`: the published text, last updated 2026-09-03,
  unchanged and compiled in.
- `/docs` and `/docs/{slug}`: the public Coder docs, compiled in. The
  install page reads the `stable` and `rc` channel pointers.
- `/blog` and `/blog/{slug}`.
- `/desktop`: new. Links the notarized OpenAgents for Mac `.dmg` in
  `openagentsgemini-oa-updates` and the TestFlight app.
- `/connect`, `/.well-known/apple-app-site-association`, and
  `/.well-known/assetlinks.json`: the pairing link's landing page and the
  app-association files, as the `coder` repository serves them.
- `/releases/{name}`, `/install-terminal.sh`, and `/install-terminal.ps1`:
  the release proxy and its short forms.
- `/gym` and `/gym/results/...`: the committed Terminal-Bench publication,
  digest-checked, with boards, attempts, and traces.
- `/app`: the local task browser, loopback only.

## Ported, waiting on a production backend

These pages render on a development server and say that their data needs
the production backend. Each reads through the `Backend` trait
(`crates/openagents-web/src/backend.rs`), which has no production
implementation yet:

| Page | Backend method | Production source |
| --- | --- | --- |
| `/traces`, `/trace/{key}` | `traces`, `trace` | The trace intake's table. |
| `/forum`, `/forum/f/{slug}`, `/forum/t/{id}` | `forum_boards`, `forum_board`, `forum_topic` | The forum's boards, topics, and posts. |
| `/earn`, `/weights`, `/qa` | `dashboard` | The fleet coordinator's status, the service's receipts, and the QA claim registry. |
| `/u/{login}` | `profile` | The account store. |
| `/ask` questions | `answer` | Chat with OpenAgents. |
| Homepage credit line | `new_account_credit_cents` | Service configuration. |

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
- Live streams: the WebSockets behind `/earn`, `/weights`, and `/qa`, and
  their recorded replays.
- Avatars (`/u/{login}/avatar`), calendar booking, `/map`, `/components`,
  `/showcase`, deployments, and page-view analytics.

## Before deploy

1. Implement `Backend` against production data, or decide to launch
   without the backend-waiting pages and drop their header and footer links.
2. Package and host the binary (for example on Cloud Run) with
   `--listen 0.0.0.0:PORT --public-host openagents.com`, behind TLS.
3. Serve the association files at `openagents.com` with no redirect, and
   check that iOS and Android still verify `/connect`.
4. Keep every URL that the `coder` repository serves and this port serves
   the same: the install command (`/releases/install-terminal.sh`), the
   pairing link (`/connect`), and the legal pages.
5. Update `about.md` in the docs: it says Coder Desktop has no public
   download, which `/desktop` now contradicts. The owner decides that copy.
6. Update `/desktop` for each desktop release; it links version 0.1.0.
7. When this server serves openagents.com, update the `INVARIANTS.md` row
   for the connect link, which names `coder-serve` as the server of
   `/connect`.
8. Cut DNS over from the `coder` deployment, then retire its site routes.
