# Login & Auth Audit — current situation + email sign-in plan

Date: 2026-06-16
Scope: `apps/openagents.com` (the live `openagents.com` Cloudflare Worker + Foldkit web app).
Reason: `/login` "does nothing"; we want an email magic-link login page (like the
ui.sh "Send sign-in link" screenshot) while preserving access gating.

## TL;DR

- **Auth server:** we self-host **OpenAuth** (`@openauthjs/openauth`) — its
  `issuer()` runs **inside our own openagents.com Cloudflare Worker** (mounted at
  `OPENAUTH_ISSUER_URL`, i.e. `auth.openagents.com`). We are **not** on WorkOS in
  this monorepo (WorkOS was the *deprecated* Laravel site). It is OpenAuth.
- **Only login method wired today: "Log in with GitHub"** (OpenAuth
  `GithubProvider` → GitHub OAuth). There is **no email/magic-link login yet.**
- **`/login` genuinely does nothing:** there is no served `/login` page. The
  Worker's document allowlist (`knownDocumentPathPatterns` in
  `workers/api/src/worker-routes.ts`) does **not** include `/login`, so
  `shouldRedirectUnknownDocumentToHome` **302-redirects `/login` → `/`**. The real
  login entry point is the **"Log in with GitHub" button on the home page**, which
  links to the Worker route **`/login/github`** (the OAuth start). There is no SPA
  `Login` route or page.
- **"Approved users" gating — the honest situation:** **login itself is open** to
  any GitHub account (the issuer's `allow` only restricts redirect *hostnames*, not
  *users*; `success` upserts whatever GitHub user signs in). Access is gated
  **downstream, in the app**, by team membership / admin flags — see §4. The
  broad "invite-required" hard gate **exists in code but is currently a no-op
  stub** (`loggedInPermissionGate` always returns `Allowed`).
- **Email is ready to use:** we already have **Resend** wired (`workers/api/src/email.ts`,
  `ResendEmailSender`, `RESEND_API_KEY/FROM/REPLY_TO` config, `@openagentsinc/email-templates`).
  Adding email sign-in is a **wiring task, not new infrastructure.**

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

## 5. Email magic-link: do we have it? (No — but everything to build it is here)

- **Auth email login: not present.** Only GitHub is wired into the issuer.
- **Send infrastructure: present.** `workers/api/src/email.ts` (Resend sender,
  `ResendEmailSender`, `EmailProvider = resend|gmail`), `RESEND_*` config, webhook
  handling (`resend-webhooks.ts`), and the `@openagentsinc/email-templates` package.
  These are used today for prelaunch/billing/operator/CRM email — not auth.
- **OpenAuth supports email natively:** `@openauthjs/openauth/provider/code`
  (`CodeProvider` — email OTP) or a custom magic-link provider. The provider's
  `sendCode` hook is where we call our Resend `email.ts`.

## 6. Plan — ship the screenshot's email `/login` page, preserve gating

Recommended, smallest-correct path:

1. **Serve `/login`.**
   - Add `/^\/login$/` to `knownDocumentPathPatterns` (`worker-routes.ts`) so the
     Worker serves the SPA there (mirrors the recent `/components`, `/business` fix).
   - Add a `Login` route in `apps/web/src/route.ts` + `routing/startup.ts`
     (logged-out allowlist) + a `page/loggedOut/page/login.ts` view: branded dark
     page, **email field + "Send sign-in link"** button (the screenshot), with the
     existing GitHub button kept as a secondary option.
2. **Wire an email provider into the OpenAuth issuer.**
   - Add `CodeProvider` (email code) or a magic-link provider to
     `makeAuthIssuer`'s `providers`, with `sendCode`/send wired to `email.ts`
     (Resend) + a new `@openagentsinc/email-templates` "sign-in link/code" template.
   - Extend the `success` callback to accept `provider === 'email'`: resolve/ upsert
     the user by **verified email** (mirror `upsertGitHubUser`), then issue the same
     `user` session. (Decide the userId scheme for email-only users.)
3. **Front the page to the flow.**
   - "Send sign-in link" → a Worker endpoint that starts the OpenAuth email flow
     (issue code/link, send via Resend) → user clicks link / enters code →
     `/auth/...callback` → session. Reuse `oa_login_return_to`.
4. **Preserve gating (key requirement).**
   - Email login only **authenticates**; **authorization is unchanged** — the
     downstream `authHasCoreTeamAccess` / `isAdmin` / onboarding gates still apply,
     so email sign-in does **not** widen who can access the real product.
   - **Decision needed:** do we also want a **hard pre-app allowlist** ("only
     approved emails can even sign in")? If yes, either (a) **re-enable
     `loggedInPermissionGate`** (it's already stubbed to allow-all — make it deny
     non-approved and route to Invite), or (b) **reject non-approved emails in the
     issuer `success` callback** (cleaner: stops a session from ever issuing). (a)
     keeps a friendly in-app "request access" page; (b) is a harder server gate.
     Today neither denies — login is open + product gated. Pick the posture before
     building.

## 7. Decisions to confirm

- **Open login + product gating (status quo) vs hard email allowlist?** (§6.4)
- **Email OTP code vs magic-link?** OpenAuth `CodeProvider` is OTP-code by default;
  magic-link is a small custom provider. The screenshot implies a **link** ("Send
  sign-in link").
- **Custom `/login` page vs OpenAuth's built-in themed UI?** The screenshot wants
  our branded page; OpenAuth can also render its own (themed) email UI at the
  issuer. Custom page = more control, slightly more wiring.
- **Resend sending domain / deliverability** for auth email (separate from
  marketing sends; auth email must be reliable + not rate-limited with CRM email).

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
