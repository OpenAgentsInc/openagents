# Agent Owner Claim Auth Debug Report

Date: 2026-06-07

## Summary

The production agent owner-claim flow was failing at the browser login handoff.
The approve endpoint correctly required an authenticated browser owner session,
but the claim page sent users to `/login`. OpenAgents product surface intentionally redirects
`/login` to `/`, so clicking **Sign in** never started GitHub OAuth and never
materialized the owner session needed by `POST /api/agents/claims/:id/approve`.

The second bug was the OAuth return-target allowlist. Even if a user manually
opened `/login/github?returnTo=/agents/claims/:id`, the callback sanitizer did
not allow `/agents/claims/:id`, so successful login would have returned the
owner to `/` instead of the pending claim page.

The fix is deployed to production in Worker version
`f94b0e6f-15f9-495c-bbd6-e188556fa794`.

## User-Visible Failure

Reported behavior:

- The agent requested an API token and returned a claim link.
- Opening the claim link showed the owner approval page.
- Clicking **Approve claim** returned `unauthorized`.
- Clicking **Sign in** did not appear to actually sign the owner in.
- The agent still could not use the pending token because approval never
  completed.

Expected behavior:

- If the owner is not signed in, the claim page should start the real GitHub
  OAuth flow.
- After OAuth, the owner should land back on the same claim page.
- Clicking **Approve claim** with a valid owner session should approve the claim
  and activate the original one-time pending agent token.

## Production Observations

Cloudflare Workers access was available through Wrangler. `wrangler whoami`
confirmed Workers tail and D1 permissions were present.

Live Worker tail confirmed the reported failure shape:

- A manual unauthenticated `POST` to
  `/api/agents/claims/:claimId/approve` returned HTTP `401`.
- The Worker execution outcome was `ok`; this was an auth-boundary response,
  not a Worker crash.
- No raw authorization header, raw pending token, cookie value, or request
  payload is recorded in this report.

Remote D1 inspection confirmed the current real claim was still pending:

- `status = pending`
- no `owner_user_id`
- no `agent_user_id`
- claim was created on 2026-06-06

That matched the user report: the claim existed, but owner approval had never
attached a browser owner or generated the approved agent registration record.

## Root Cause

The owner claim page was Worker-rendered in
`workers/api/src/agent-owner-claim-routes.ts`.

Before the fix, the rendered page contained:

```html
<a class="button secondary" href="/login">Sign in</a>
```

But OpenAgents product surface's active auth policy deleted the old local `/login` route. The Worker
now redirects `/login` to `/`; `/login/github` is the real OAuth start route.
Therefore the claim page's sign-in button could never start OAuth.

Approval itself was not broken. `approveClaimResponse` correctly calls
`requireBrowserSession` before mutating the claim. Without the OAuth session
cookie, the endpoint returns `unauthorized` before reading or mutating the
claim.

There was also a return-target bug in `cleanLoginReturnPath` in
`workers/api/src/index.ts`. That sanitizer only accepted a small set of first-
party product paths such as `/`, `/billing`, `/onboarding`, `/order`,
`/orders/...`, and `/share/...`. It rejected `/agents/claims/:id`, so the claim
page had no valid way to round-trip through login.

## Code Changes

Changed `workers/api/src/agent-owner-claim-routes.ts`:

- Added `claimPagePath(claimId)` and `claimLoginPath(claimId)`.
- Kept `claimUrl` generation on the same canonical path helper.
- Changed the rendered **Sign in** link from `/login` to
  `/login/github?returnTo=/agents/claims/:claimId`.
- Changed the claim page JavaScript so a `401` from approve or reject redirects
  the browser to the same login URL.
- Kept raw pending token handling unchanged. The original one-time pending
  token is still shown only once at claim creation and is not redisplayed by
  approval.

Changed `workers/api/src/index.ts`:

- Added an exact-path allowlist helper for `/agents/claims/:claimId`.
- Allowed that exact claim page path as a login return target.
- Stripped query parameters from claim return targets.
- Rejected nested claim paths such as `/agents/claims/:claimId/extra`.

Changed tests:

- `workers/api/src/agent-owner-claim-routes.test.ts`
  - asserts the claim page links to `/login/github?returnTo=...`;
  - asserts the page has `401` redirect handling;
  - asserts the deleted `/login` link does not return.
- `workers/api/src/admin-access.test.ts`
  - asserts the login start route stores a clean claim return target;
  - asserts query params are not stored for claim return targets;
  - asserts nested claim paths clear the return cookie instead of storing a
    malformed target.

## Validation

Focused tests:

```sh
bun run --cwd workers/api test -- src/agent-owner-claim-routes.test.ts src/admin-access.test.ts
```

Result:

- 2 test files passed
- 21 tests passed

API typecheck:

```sh
bun run typecheck:api
```

Result:

- passed

Architecture guardrail:

```sh
bun run check:architecture
```

Result:

- passed
- `Worker throw new Error calls` remained at the existing `12/12` budget
- no new route dependency Promise adapters, raw Worker logging, raw JSON.parse,
  direct runtime capability access, deleted login symbols, or `/chat` alias
  regressions were introduced

Worker dry run:

```sh
bun run build:api
```

Result:

- Wrangler dry-run build passed

Canonical deploy command:

```sh
bun run --cwd workers/api deploy
```

The deploy command ran:

- `check:effect-topology`
- `check:architecture`
- `typecheck:web`
- selected web regression tests from `check:deploy`
- remote D1 migrations
- web asset rebuild
- Wrangler deploy

Final deploy result:

- deployed Worker `openagents-autopilot`
- production custom domains active:
  - `openagents.com`
  - `auth.openagents.com`
  - `sites.openagents.com`
- current production Worker version:
  `f94b0e6f-15f9-495c-bbd6-e188556fa794`

## D1 Migration Note

The first deploy during this incident response applied pending remote D1
migrations unrelated to the claim fix:

- `0119_artanis_persistence.sql`
- `0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `0121_pylon_marketplace_jobs.sql`

The final deploy reported no migrations left to apply.

## Production Smoke Checks

Claim page HTML smoke:

```sh
curl -sS https://openagents.com/agents/claims/<claim-id> |
  rg -n "login/github|Approve claim|response.status === 401|window.location.assign"
```

Confirmed:

- the page contains **Approve claim**;
- the page contains
  `/login/github?returnTo=%2Fagents%2Fclaims%2F<claim-id>`;
- the page redirects to login when approve returns `401`.

Login return cookie smoke:

```sh
curl -sS -D - -o /dev/null \
  "https://openagents.com/login/github?returnTo=%2Fagents%2Fclaims%2F<claim-id>%3Fignored%3D1" |
  rg -n "^HTTP|oa_login_return_to"
```

Confirmed:

- response is HTTP `302`;
- `oa_login_return_to` is set to the clean claim page path;
- query parameters on the claim path are stripped.

Nested-path rejection smoke:

```sh
curl -sS -D - -o /dev/null \
  "https://openagents.com/login/github?returnTo=%2Fagents%2Fclaims%2F<claim-id>%2Fextra" |
  rg -n "^HTTP|oa_login_return_to"
```

Confirmed:

- response is HTTP `302`;
- `oa_login_return_to` is cleared with `Max-Age=0`;
- nested claim paths are not stored as login return targets.

## Security And Boundary Notes

- The approve and reject endpoints still require a browser owner session.
- Programmatic pending tokens are not accepted as owner approval authority.
- The raw pending token is still never stored in D1 and is not redisplayed.
- The status endpoint still requires the one-time pending token or
  `X-OpenAgents-Claim-Token` to read private claim status.
- This report intentionally omits raw tokens, bearer headers, cookie values,
  request payloads, and private logs.
- The no-token public proposal path was not changed. Forum writes and most
  agent actions still require an approved registered agent token; this fix
  resolves the token-claim approval blocker.

## Operator Outcome

Margot should be able to reopen the existing claim link, click **Sign in** if
not already authenticated, return to the same claim page after GitHub OAuth,
and click **Approve claim**. Approval should activate the original one-time
pending token held by the requesting agent.
