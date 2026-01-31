# Website authentication (Better Auth)

The OpenAgents website uses [Better Auth](https://better-auth.com) for sign-in and sessions. It supports:

- **Email/password** — sign up and sign in with email (stored in DB); `emailAndPassword.enabled: true`.
- **GitHub OAuth** (“Sign in with GitHub”)
- **Cloudflare D1** for user/session storage when deployed (binding `DB` in `wrangler.jsonc`)
- **Optional URL-based DB** for local dev when D1 is not available (`BETTER_AUTH_DATABASE_URL`)

## Setup

### 1. Environment variables

Create a `.env` file (e.g. copy from the block below) and set:

| Variable | Required | Description |
|----------|----------|-------------|
| `PUBLIC_SITE_URL` | Yes | Site origin (e.g. `https://openagents.com` or `http://localhost:4321`). Must be `PUBLIC_` so the auth client can use it in the browser. |
| `BETTER_AUTH_SECRET` | Yes (prod) | Secret for signing (min 32 chars). In dev, a default is used if unset. |
| `GITHUB_CLIENT_ID` | For GitHub sign-in | From [GitHub OAuth Apps](https://github.com/settings/developers). |
| `GITHUB_CLIENT_SECRET` | For GitHub sign-in | From the same OAuth app. Callback URL: `{PUBLIC_SITE_URL}/api/auth/callback/github`. |
| `BETTER_AUTH_DATABASE_URL` | Optional | Used when D1 is not available (e.g. local). e.g. `libsql:file:./local.db` or a Postgres URL. |

Example `.env`:

```env
PUBLIC_SITE_URL=https://openagents.com
BETTER_AUTH_SECRET=
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=
BETTER_AUTH_DATABASE_URL=
```

### 2. D1 (production / Cloudflare)

1. Create a D1 database:
   ```bash
   npx wrangler d1 create website-auth
   ```
2. In `wrangler.jsonc`, set `database_id` (and optionally `preview_database_id`) under `d1_databases` for the `DB` binding.
3. Apply Better Auth schema to D1:
   ```bash
   npx @better-auth/cli generate
   # or use the project’s DB URL / D1 and run the generated migration
   ```
   If the CLI does not support D1 directly, run the SQL it generates against your D1 database (e.g. via `wrangler d1 execute`).

### 3. Local development

- With **D1**: use `platformProxy` (already in `astro.config.mjs`) and ensure `wrangler.jsonc` has a valid D1 binding; Wrangler will inject `DB` in dev.
- Without D1: set `BETTER_AUTH_DATABASE_URL` (e.g. `libsql:file:./local.db`) so Better Auth can store users/sessions.

## Code layout

- **Server**: `src/lib/auth.ts` — `createAuth(db)` for D1 (Kysely + kysely-d1), default `auth` for URL-based or no DB. Used in the API route.
- **API route**: `src/pages/api/auth/[...all].ts` — mounts Better Auth at `/api/auth/*`; uses `createAuth(runtime.env.DB)` when `DB` is present, else default `auth`.
- **Client**: `src/lib/auth-client.ts` — `createAuthClient` with `baseURL` from `PUBLIC_SITE_URL`; exports `signIn`, `signOut`, `signUp`, `getSession` for use in browser scripts.

## Sign-in / sign-up pages

- **`/login`** — Email/password form + "Log in with GitHub". Uses `signIn.email()` and `signIn.social({ provider: 'github' })`.
- **`/sign-up`** — Name, email, password form + "Sign up with GitHub". Uses `signUp.email()` and `signIn.social()` for GitHub.

## Header sign-in / sign-out

The header uses a client script that calls `getSession()`. If there is a session, it shows “Sign out” (calls `signOut()` then reloads). If not, it shows “Sign in with GitHub” linking to `/login`.

## References

- [Better Auth](https://better-auth.com) — configuration, adapters, social providers.
- [Astro authentication guide](https://docs.astro.build/en/guides/authentication/).
- [Better Auth Astro example](https://better-auth.com/docs/examples/astro).
