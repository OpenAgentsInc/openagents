# Login & Auth Audit — current situation + email sign-in plan

Date: 2026-06-16 (status updated 2026-06-16, post-implementation)
Scope: `apps/openagents.com` (the live `openagents.com` Cloudflare Worker + Foldkit web app).
Reason: `/login` "did nothing"; we wanted an email sign-in page (like the
ui.sh "Send sign-in link" screenshot) while preserving access gating.

> **Strategy home:** the single, ordered "what we do when" sequencing for auth +
> all product email now lives in
> `docs/auth/2026-06-16-cloudflare-email-automation-audit.md` → **"Unified Email &
> Auth Strategy — what we do when."** This document is the auth/login subsystem
> reference; that document owns the cross-cutting roadmap. Keep them consistent.

## Status (SHIPPED — 2026-06-16)

The email sign-in plan in this audit **has been implemented and deployed** (issue
#5111, worker version `d9113b02`). What changed from the "current situation"
below:

- **`/login` is now a real branded SPA page** (`apps/web/src/page/login.ts`,
  `LoginRoute`/`loginRouter`), no longer a 302 to `/`. It offers email one-time
  code sign-in + GitHub, over the constellation-network animation. `/^\/login$/`
  is in `knownDocumentPathPatterns`.
- **Email sign-in is live as OTP code (not magic-link):** OpenAuth `CodeProvider`
  + `CodeUI` is registered in `makeAuthIssuer`; `/login/email` starts the code
  flow; `success()` accepts `provider: 'code'`, upserts the user by verified
  email, and issues the same `user` session. `UserSubject` was widened to
  `provider: 'github' | 'email'` with optional `githubId`/`login`.
- **Auth email transport (interim): direct Resend.** `sendSignInCodeEmail` posts
  the code straight to the Resend API, deliberately decoupled from the
  `EmailService` CRM/marketing ledger. This is the **interim** transport; the
  target state (first-party Cloudflare auth sender on a dedicated subdomain) is
  sequenced in the unified strategy doc — see §6/§7 and that doc's Phase 3.
- **Auth OTP hardening shipped (#5120):** code send/resend requests on
  `/code/authorize` now pass a first-party D1-backed guard before OpenAuth's
  `CodeProvider.sendCode` hook can send mail: 8 sends per IP per 10 minutes, 4
  sends per normalized target email per 10 minutes, and 180 total sends per hour.
  The guard stores hashed IP/email bucket subjects only, fails closed with
  no-store retry responses if storage is unavailable, and returns `Retry-After`
  on throttled attempts. We verified OpenAuth 0.4.3 persists code-provider state
  for 24 hours with no TTL option; our layer stamps signed-in claims with a
  10-minute server-side expiry and rejects stale code sessions in `success()`
  before issuing a user session. Sender misconfiguration/unavailability returns
  the same retry form instead of storing a code state, and the email subject no
  longer contains the raw code.
- **Gating preserved:** login still only *authenticates*; authorization stays
  downstream (`authHasCoreTeamAccess` / `isAdmin` / onboarding). Email login does
  not widen access. Invariant recorded in `apps/openagents.com/INVARIANTS.md`
  ("Login Surface").

The sections below are retained as the original audit (the "before" state and the
rationale) for history; read the Status block and §6/§7 for what is true now.

## TL;DR (original audit — "before" state)

- **Auth server:** we self-host **OpenAuth** (`@openauthjs/openauth`) — its
  `issuer()` runs **inside our own openagents.com Cloudflare Worker** (mounted at
  `OPENAUTH_ISSUER_URL`, i.e. `auth.openagents.com`). We are **not** on WorkOS in
  this monorepo (WorkOS was the *deprecated* Laravel site). It is OpenAuth.
- **(NOW SHIPPED) At time of writing, only "Log in with GitHub" was wired**
  (OpenAuth `GithubProvider`). Email OTP login has since been added — see Status.
- **(NOW SHIPPED) `/login` used to 302 to `/`.** It is now a served SPA page —
  see Status.
- **"Approved users" gating — the honest situation (still true):** **login itself
  is open** to any GitHub or email account (the issuer's `allow` only restricts
  redirect *hostnames*, not *users*; `success` upserts whoever signs in). Access
  is gated **downstream, in the app**, by team membership / admin flags — see §4.
  The broad "invite-required" hard gate **exists in code but is a no-op stub**
  (`loggedInPermissionGate` always returns `Allowed`).
- **Email infra:** **Resend** is wired (`workers/api/src/email.ts`,
  `ResendEmailSender`, `RESEND_API_KEY/FROM/REPLY_TO` config,
  `@openagentsinc/email-templates`). Adding email sign-in was a wiring task, not
  new infrastructure — and it is now done.

## 1. The auth server & stack

- **Library:** `@openauthjs/openauth` (imported in `workers/api/src/index.ts`):
  `issuer`, `createClient`, `GithubProvider`, `createSubjects`, `THEME_OPENAUTH`.
- **Where it runs:** the issuer is constructed in `makeAuthIssuer(env)` and served
  by our own Worker (a request to the issuer host is routed to
  `makeAuthIssuer(env).fetch(...)` via `makeIssuerAwareFetch`). So OpenAuth is
  **embedded in `openagents.com`**, not a third-party hosted service.
- **Config** (`workers/api/src/config.ts`): `OPENAUTH_ISSUER_URL`,
  `OPENAUTH_CLIENT_ID`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`,
  `OPENAGENTS_APP_URL`. (Resend keys also live here.)
- **Storage:** OpenAuth uses **D1** (`makeD1Storage`, `workers/api/src/auth/openauth-storage.ts`).
  The GitHub identity token is cached in the **`AUTH_STORAGE` KV**
  (`githubIdentityTokenKey`).
- **Subjects:** `createSubjects({ user: ... })` — a `user` subject carries
  `provider: 'github'`, `githubId`, `login`, `userId`, primary verified `email`.
- **Sessions & cookies:** `workers/api/src/auth/session.ts` + `auth-cookies.ts`;
  `SESSION_MAX_AGE_SECONDS` TTL. The SPA bootstraps auth by fetching
  **`GET /api/auth/session`** (see `apps/web/src/main.ts`), which returns
  `{ authenticated, bootstrap }`.
- **Issuer UI:** OpenAuth ships its own themed UI (`THEME_OPENAUTH`, title
  "OpenAgents"). Today that UI is essentially just the GitHub button hop.

## 2. The current login flow (step by step)

1. Home page renders **"Log in with GitHub"** (`apps/web/src/page/loggedOut/page/home.ts`,
   `githubLoginButton`, `href = /login/github`).
2. `GET /login/github` (Worker) → OpenAuth authorize → redirect to
   `github.com/login/oauth/authorize` (scopes `GITHUB_LOGIN_SCOPES`).
3. GitHub redirects back to **`/github/callback`** → OpenAuth exchanges the code.
4. **`success` callback** (`index.ts` ~line 2138): requires `provider === 'github'`,
   fetches `GET /user` + `/user/emails`, builds the subject
   (`githubUserToSubject` + primary verified email), `upsertGitHubUser(D1)`, stores
   the identity token in `AUTH_STORAGE`, and issues the `user` session.
5. Redirect back to the app using `oa_login_return_to` / `oa_login_origin` cookies.
6. SPA loads, calls `/api/auth/session`, gets the bootstrap, and routes.

## 3. Why `/login` does nothing

- `/login` is **not** in `knownDocumentPathPatterns` (`worker-routes.ts`). For a
  GET that accepts HTML, `shouldRedirectUnknownDocumentToHome` returns true for any
  path not on that list (and not under `/api`, `/assets`, `/auth`, `/checkout`,
  `/openagents-agent-claim`) → **302 to `/`**.
- There is **no `Login` route** in `apps/web/src/route.ts` and **no login page** in
  the SPA. Login lives on the home page + the `/login/github` Worker route.
- Net: typing `/login` bounces to home; nothing "logs in."

## 4. Access gating — what "approved users" actually means today

Authentication (who can sign in) and authorization (what they can see) are
**separate** here:

- **Authentication = open.** The issuer `allow` callback only checks the redirect
  **hostname** (`openagents.com` / `auth.openagents.com` / localhost). `success`
  upserts **any** GitHub user. There is **no email/login allowlist at the auth
  layer.**
- **Authorization = downstream, in `apps/web/src/product-policy.ts`:**
  - `loggedInPermissionGate(auth)` → **currently a no-op** that always returns
    `BrowserPermissionAllowed`. The hard "invite-required" redirect machinery
    exists (`InviteRoute`, `StartupRedirectToInvite`, `startupRouteForInviteRequired`
    in `routing/startup.ts`) but is **dormant** because the gate never denies.
  - **Operator / workroom / chat / billing surfaces** require
    `authHasCoreTeamAccess(auth)` (core-team membership) via `loggedInWorkroomAllowed`
    + onboarding complete (`routeAllowedForLoggedInAuth`).
  - **Admin surfaces:** `auth.isAdmin && onboardingIsComplete`.
  - **Owner-only (e.g. mullet):** admin && `email === 'chris@openagents.com'`.
  - Onboarding completion (`onboardingIsComplete(auth.onboarding)`) gates the full
    logged-in experience; incomplete onboarding → onboarding route.
- **So "approved users" today** = anyone can authenticate, but the real product
  (workrooms, chat, billing, admin) is gated to **core-team members / admins /
  owner**. Non-approved authenticated users get the limited/onboarding surface.
- **Separate identity path:** agent identity ("Claim Your Agent") is tweet-first /
  X-verification and owner-claim routes (per repo AGENTS.md), **not** the human
  login flow. Don't conflate them.

## 5. Email sign-in — SHIPPED as OTP code (not magic-link)

**Resolved.** We shipped email sign-in as an **OTP code** via OpenAuth's native
`CodeProvider` + `CodeUI` (more robust / less custom than a magic-link provider;
the UX is a 6-digit code, not a clicked link). The provider's `sendCode` hook
sends the code; `success()` accepts `provider: 'code'` and issues the session.

- **Transport (interim):** `sendSignInCodeEmail` posts the code **directly to the
  Resend HTTP API**, intentionally bypassing the `EmailService` ledger — auth
  email must be reliable and not throttled alongside CRM/marketing sends. This is
  the interim. The target transport is a **first-party Cloudflare auth sender on a
  dedicated subdomain** (e.g. `login@auth.openagents.com`), onboarded and
  smoke-tested first — see the unified strategy doc, Phase 3.
- **Abuse and expiry controls:** a dedicated auth OTP guard now wraps the
  provider send/resend route. It rate-limits by IP, normalized target email, and
  global hourly volume before a code email can be sent; stores only hashed bucket
  subjects; removes the code from the email subject; fails closed on missing
  sender config; and enforces our 10-minute session-issuance expiry even though
  the upstream OpenAuth `CodeProvider` state remains stored for 24 hours.
- **Why not magic-link:** OTP via `CodeProvider` is battle-tested in OpenAuth and
  avoids hand-rolling a link provider + token store. Revisit only if product wants
  click-to-login UX.

## 6. Plan — DONE (this is what shipped)

The smallest-correct path below was implemented (issue #5111). Status per step:

1. **Serve `/login`. — DONE.** `/^\/login$/` is in `knownDocumentPathPatterns`;
   `LoginRoute`/`loginRouter` added; the page is `apps/web/src/page/login.ts`
   (note: at top level `page/login.ts`, not `page/loggedOut/page/login.ts`, to
   avoid the deleted-simulated-login path the architecture check still bans). The
   old `/login → /` worker redirect was removed. Branded dark page with the email
   field + "Email me a code" primary and "Log in with GitHub" secondary, over the
   constellation animation.
2. **Wire email into the issuer. — DONE.** `CodeProvider(CodeUI({ sendCode }))`
   registered in `makeAuthIssuer`. `success()` accepts `provider: 'code'`,
   `emailToSubject` + `upsertEmailUser` resolve the user by verified email,
   `userId = email:<normalized>`, then the same `user` session is issued. (We used
   OTP, not a magic-link; transport is direct Resend for now — see §5.)
3. **Front the page to the flow. — DONE.** `/login/email` starts the OpenAuth code
   flow (mirrors `/login/github`); OpenAuth's themed `CodeUI` collects email then
   code; on success → `/auth/callback` → session. The header "Log in" popover now
   offers both email and GitHub.
4. **Preserve gating. — DONE (and the open decision is resolved).**
   - Email login only **authenticates**; downstream `authHasCoreTeamAccess` /
     `isAdmin` / onboarding gates are unchanged, so email sign-in does **not**
     widen who can access the real product. Recorded as an invariant
     ("Login Surface") in `apps/openagents.com/INVARIANTS.md`.
   - **Posture decision (resolved, owner-confirmed): open authentication + downstream
     product gating (status quo).** We did **not** add a hard pre-app email
     allowlist and did **not** un-stub `loggedInPermissionGate`. Anyone can get a
     session; the product stays gated to core-team/admins/owner. If we later want
     a hard gate, prefer rejecting non-approved emails in the issuer `success`
     callback (fails closed before a session issues) over re-enabling the in-app
     gate.

## 7. Decisions — resolved

- **Open login + product gating vs hard email allowlist?** → **Open auth +
  downstream gating** (status quo). No hard pre-app email allowlist. (§6.4)
- **Email OTP code vs magic-link?** → **OTP code** via OpenAuth `CodeProvider`
  (robust, native). Magic-link deferred unless product asks for click-to-login.
- **Custom `/login` page vs OpenAuth's built-in UI?** → **Both:** our branded
  `/login` launcher page + OpenAuth's themed `CodeUI` for the email/code entry.
- **Auth-email sender domain / deliverability (still open, owned elsewhere):**
  interim is direct Resend with the existing `RESEND_FROM`. Target is a dedicated
  Cloudflare auth sender/subdomain, preserving the #5120 OTP throttle, expiry,
  no-enumeration, and fail-closed controls. This is now sequenced in the unified
  strategy doc (`2026-06-16-cloudflare-email-automation-audit.md`, Phase 1→3) —
  track it there, not here.

## 8. File map

- Issuer + success + GitHub provider: `workers/api/src/index.ts` (`makeAuthIssuer`,
  `success`, `subjects`, `/login/github`, `/github/callback`).
- OpenAuth D1 storage: `workers/api/src/auth/openauth-storage.ts`.
- Sessions / cookies: `workers/api/src/auth/session.ts`, `workers/api/src/auth-cookies.ts`.
- Session bootstrap endpoint: `workers/api/src/index.ts` (`/api/auth/session`).
- Document-serving allowlist (why `/login` 302s): `workers/api/src/worker-routes.ts`.
- Login button / home page: `apps/web/src/page/loggedOut/page/home.ts`.
- Access gating: `apps/web/src/product-policy.ts`
  (`loggedInPermissionGate`, `authHasCoreTeamAccess`, `loggedInWorkroomAllowed`,
  `loggedInAdminAccessAllowed`), `apps/web/src/routing/startup.ts`.
- Email send infra (reuse for magic-link): `workers/api/src/email.ts`, `config.ts`
  (`RESEND_*`), `@openagentsinc/email-templates`.
- Config keys: `workers/api/src/config.ts`.
