# OpenAgents Web

The **OpenAgents** web app — [openagents.com](https://openagents.com). Public feed, communities, Hatchery (flow canvas), and managed OpenClaw chat; auth and billing via WorkOS + Convex.

## What’s in this app

- **Home** — Nostr-powered feed and community discovery (NostrGridHome).
- **Communities** — Browse and open communities; posts, events, replies.
- **Hatchery** — Flow-first canvas (SVG graph) and waitlist for the upcoming “create your OpenClaw” flow.
- **Chat** — `/chat` uses AI SDK + Assistant UI with server-side tools that call the OpenAgents API (`/api/openclaw/*`) for managed OpenClaw (provisioning, status, tools).
- **Auth** — WorkOS AuthKit (redirect-based sign-in/sign-up, session handling, protected routes).
- **Backend** — Convex (database, server functions, HTTP endpoints). Nostr sync, waitlist, users, billing, and OpenClaw control live in `convex/`.

## Stack

- **Frontend**: [React](https://react.dev/), [TanStack Start](https://tanstack.com/start) (file-based routing), [Tailwind](https://tailwindcss.com/)
- **Backend**: [Convex](https://convex.dev/) (DB, server logic, auth validation)
- **Auth**: [WorkOS AuthKit](https://authkit.com/)
- **Hosting**: [Cloudflare Workers](https://workers.cloudflare.com/) (Wrangler); production Convex deployment separate

## Get started

1. **Install**

   ```bash
   npm install
   ```

2. **Env**

   ```bash
   cp .env.local.example .env.local
   ```

   Then set WorkOS and Convex values in `.env.local`:
   - WorkOS: Client ID, API Key, Cookie password (min 32 chars), Redirect URI `http://localhost:3000/callback`
   - Get keys from [WorkOS dashboard](https://workos.com/); add `http://localhost:3000/callback` as a redirect URI there

3. **Convex**

   ```bash
   npx convex dev
   ```

   This creates/uses a Convex deployment and updates `.env.local`. Set WorkOS in Convex so it can validate JWTs:

   ```bash
   npx convex env set WORKOS_CLIENT_ID <your_client_id>
   npx convex env set WORKOS_API_KEY <your_api_key>
   ```

4. **Run**

   ```bash
   npm run dev
   ```

   Starts Vite (frontend) and Convex dev in parallel. Open [http://localhost:3000](http://localhost:3000).

## Deploy

- **Web (Cloudflare Workers)**  
  Builds with prod Convex URL and deploys the **openagents-web-app** Worker:

  ```bash
  npm run deploy
  ```

  Uses `.env` / `.env.production` for `VITE_CONVEX_URL` (default `https://successful-mongoose-647.convex.cloud`). Production URL: **https://openagents-web-app.openagents.workers.dev** (and [openagents.com](https://openagents.com) when configured).

- **Convex (prod)**  
  Use prod WorkOS env so AuthKit and deployment match:

  ```bash
  npm run deploy:convex
  ```

  This loads `.env.production` (prod WorkOS keys + `OPENAGENTS_WEB_URL=openagents.com`) and runs `npx convex deploy --yes`. Do not run `npx convex deploy` with only dev env or you’ll hit WorkOS env mismatch.

### OpenClaw chat (beta)

`/chat` uses server-side tools that call the Rust API at `/api/openclaw/*`. To enable them, set the internal auth secret on the Worker:

```bash
npx wrangler secret put OA_INTERNAL_KEY
```

Without it, OpenClaw tools can return HTTP 500.

## Docs

- **Flow / Hatchery**: [docs/flow-conversion-plan.md](docs/flow-conversion-plan.md)
- **OpenClaw on openagents.com**: [docs/openclaw-on-openagents-com.md](docs/openclaw-on-openagents-com.md)
- **Repo overview**: [README.md](../../README.md) and [AGENTS.md](../../AGENTS.md) in the repo root
